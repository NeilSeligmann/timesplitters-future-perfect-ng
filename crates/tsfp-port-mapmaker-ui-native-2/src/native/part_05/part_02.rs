use super::*; // portreg: replaces=tsfp-ports/mapmaker_ui_lane
#[rustfmt::skip] mod unformatted { use super::*;

/// 0x0032CE00: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f32ce00<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x32ce00;
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
        { crate::pose::cover(0x32ce00, previous, pc); previous = pc; }
        match pc {
            0x32ce00 => {
                eax = m.read32(ebx.wrapping_add(0x44));
                push!(ebp);
                ebp = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x32ce4b; continue; }
                pc = 0x32ce0a; continue;
            }
            0x32ce0a => {
                push!(esi);
                push!(edi);
                esi = 0x4158;
                edi = ebx.wrapping_add(0x48);
                pc = 0x32ce14; continue;
            }
            0x32ce14 => {
                fa = ((((m.read8(edi))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if (fa > fb) { pc = 0x32ce3e; continue; }
                pc = 0x32ce19; continue;
            }
            0x32ce19 => {
                fa = ((((m.read8(edi.wrapping_add(0x5)))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x32ce3e; continue; }
                pc = 0x32ce1f; continue;
            }
            0x32ce1f => {
                eax = m.read32(0x7844a8);
                eax = m.read32(esi.wrapping_add(eax));
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0x28))) as u32)) & 0xff);
                esi = esi.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x32ce3e; continue; }
                pc = 0x32ce32; continue;
            }
            0x32ce32 => {
                ecx = m.read32(eax.wrapping_add(0x4));
                push!(ecx);
                call!(0x96810);
                pc = 0x32ce3b; continue;
            }
            0x32ce3b => {
                esp = esp.wrapping_add(0x4);
                pc = 0x32ce3e; continue;
            }
            0x32ce3e => {
                eax = m.read32(ebx.wrapping_add(0x44));
                ebp = ebp.wrapping_add(1);
                edi = edi.wrapping_add(0x6);
                fa = ebp;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x32ce14; continue; }
                pc = 0x32ce49; continue;
            }
            0x32ce49 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x32ce4b; continue;
            }
            0x32ce4b => {
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0032D9B0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f32d9b0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x32d9b0;
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
        { crate::pose::cover(0x32d9b0, previous, pc); previous = pc; }
        match pc {
            0x32d9b0 => {
                ecx = m.read32(esp.wrapping_add(0x4));
                eax = ecx;
                eax = eax.wrapping_shr((((((((0x18i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = ecx;
                edx = edx.wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = edx.wrapping_shl((((((((0x11i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = eax.wrapping_shl((((((((0x19i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = (edx & 0xff0000);
                eax = (eax & 0xff000000u32);
                eax = (eax | edx);
                edx = ecx;
                edx = edx.wrapping_shr((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = edx.wrapping_shl((((((((0x9i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = (edx & 0xff00);
                eax = (eax | edx);
                ecx = (ecx & 0xff);
                eax = (eax | ecx);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}
}
pub use unformatted::*;
