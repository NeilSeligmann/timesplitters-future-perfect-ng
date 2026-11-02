use super::*; // portreg: replaces=tsfp-ports/mapmaker_ui_lane
#[rustfmt::skip] mod unformatted { use super::*;

/// 0x003223A0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3223a0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3223a0;
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
        { crate::pose::cover(0x3223a0, previous, pc); previous = pc; }
        match pc {
            0x3223a0 => {
                eax = m.read32(0x7844a8);
                push!(ebx);
                push!(ebp);
                push!(esi);
                push!(edi);
                edi = ((m.read16(eax.wrapping_add(0x3068))) as u16 as u32);
                push!(0x36);
                call!(0x308300);
                pc = 0x3223b7; continue;
            }
            0x3223b7 => {
                push!(0x35);
                esi = eax;
                call!(0x308300);
                pc = 0x3223c0; continue;
            }
            0x3223c0 => {
                push!(0x37);
                esi = esi.wrapping_add(eax);
                call!(0x308300);
                pc = 0x3223c9; continue;
            }
            0x3223c9 => {
                push!(0x322300);
                push!(0);
                push!(0x42480000);
                push!(0);
                push!(0x8400);
                ebx = eax;
                call!(0x75b70);
                pc = 0x3223e3; continue;
            }
            0x3223e3 => {
                ebp = m.read32(esp.wrapping_add(0x34));
                push!(0x1);
                push!(0x1);
                push!(0x3f000000);
                push!(0x1);
                m.write32(ebp.wrapping_add(0x218), 0x1);
                call!(0x71b10);
                pc = 0x322401; continue;
            }
            0x322401 => {
                push!(0x20000);
                call!(0x71c50);
                pc = 0x32240b; continue;
            }
            0x32240b => {
                esp = esp.wrapping_add(0x34);
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0xb21);
                call!(0x3d7b0);
                pc = 0x322420; continue;
            }
            0x322420 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79460);
                pc = 0x322429; continue;
            }
            0x322429 => {
                esp = esp.wrapping_add(0x14);
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = 0x321f50;
                if (fa != fb) { pc = 0x32243a; continue; }
                pc = 0x322435; continue;
            }
            0x322435 => {
                eax = 0x3222b0;
                pc = 0x32243a; continue;
            }
            0x32243a => {
                push!(0);
                push!(eax);
                push!(0);
                push!(0x800);
                push!(0);
                push!(0xb0f);
                call!(0x3d7b0);
                pc = 0x322450; continue;
            }
            0x322450 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x322459; continue;
            }
            0x322459 => {
                push!(0x87);
                call!(0x716d0);
                pc = 0x322463; continue;
            }
            0x322463 => {
                esp = esp.wrapping_add(0x1c);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = 0x321f50;
                if (fa != fb) { pc = 0x322474; continue; }
                pc = 0x32246f; continue;
            }
            0x32246f => {
                eax = 0x3222b0;
                pc = 0x322474; continue;
            }
            0x322474 => {
                push!(0);
                push!(eax);
                push!(0);
                push!(0x800);
                push!(0x1);
                push!(0xb10);
                call!(0x3d7b0);
                pc = 0x32248a; continue;
            }
            0x32248a => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x322493; continue;
            }
            0x322493 => {
                push!(0x88);
                call!(0x716d0);
                pc = 0x32249d; continue;
            }
            0x32249d => {
                esp = esp.wrapping_add(0x1c);
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = 0x321f50;
                if (fa != fb) { pc = 0x3224ae; continue; }
                pc = 0x3224a9; continue;
            }
            0x3224a9 => {
                eax = 0x3222b0;
                pc = 0x3224ae; continue;
            }
            0x3224ae => {
                push!(0);
                push!(eax);
                push!(0);
                push!(0x800);
                push!(0x2);
                push!(0xb11);
                call!(0x3d7b0);
                pc = 0x3224c4; continue;
            }
            0x3224c4 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x3224cd; continue;
            }
            0x3224cd => {
                push!(0x89);
                call!(0x716d0);
                pc = 0x3224d7; continue;
            }
            0x3224d7 => {
                call!(0x783d0);
                pc = 0x3224dc; continue;
            }
            0x3224dc => {
                push!(0x40000000);
                call!(0x784a0);
                pc = 0x3224e6; continue;
            }
            0x3224e6 => {
                esp = esp.wrapping_add(0x20);
                push!(0);
                push!(0x322300);
                push!(0x40000000);
                push!(0);
                push!(0xffffffffu32);
                push!(0xb5e);
                call!(0x3d7b0);
                pc = 0x322503; continue;
            }
            0x322503 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x32250c; continue;
            }
            0x32250c => {
                call!(0x75e70);
                pc = 0x322511; continue;
            }
            0x322511 => {
                xmm0 = Xmm::scalar(m.read32(0x49f7e8));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ebp.wrapping_add(0x68)))).to_bits();
                ecx = m.read32(0x7bb1e8);
                esp = esp.wrapping_add(0x14);
                m.write32(esp, xmm0.lanes[0]);
                push!(ecx);
                call!(0x77320);
                pc = 0x322532; continue;
            }
            0x322532 => {
                esp = esp.wrapping_add(0x8);
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

/// 0x00322BB0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f322bb0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x322bb0;
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
        { crate::pose::cover(0x322bb0, previous, pc); previous = pc; }
        match pc {
            0x322bb0 => {
                push!(0x361710);
                push!(0);
                push!(0x42480000);
                push!(0xb3f);
                call!(0x3d7b0);
                pc = 0x322bc6; continue;
            }
            0x322bc6 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x8400);
                call!(0x75b70);
                pc = 0x322bd4; continue;
            }
            0x322bd4 => {
                push!(0x1);
                push!(0x1);
                push!(0x40000000);
                push!(0x1);
                call!(0x71b10);
                pc = 0x322be4; continue;
            }
            0x322be4 => {
                push!(0x28000);
                call!(0x71c50);
                pc = 0x322bee; continue;
            }
            0x322bee => {
                esp = esp.wrapping_add(0x28);
                push!(0);
                push!(0);
                push!(0);
                call!(0x22b90);
                pc = 0x322bfc; continue;
            }
            0x322bfc => {
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
                pc = 0x322c4a; continue;
            }
            0x322c4a => {
                esp = esp.wrapping_add(0x10);
                push!(0);
                push!(0x31dfe0);
                push!(0);
                push!(0x800);
                push!(0);
                push!(0xb40);
                call!(0x3d7b0);
                pc = 0x322c67; continue;
            }
            0x322c67 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x322c70; continue;
            }
            0x322c70 => {
                push!(0xcb);
                call!(0x716d0);
                pc = 0x322c7a; continue;
            }
            0x322c7a => {
                esp = esp.wrapping_add(0x1c);
                push!(0);
                push!(0x31dfe0);
                push!(0);
                push!(0x4000000);
                push!(0x1);
                push!(0xb41);
                call!(0x3d7b0);
                pc = 0x322c97; continue;
            }
            0x322c97 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x322ca0; continue;
            }
            0x322ca0 => {
                push!(0xcc);
                call!(0x716d0);
                pc = 0x322caa; continue;
            }
            0x322caa => {
                push!(0x40000000);
                call!(0x784a0);
                pc = 0x322cb4; continue;
            }
            0x322cb4 => {
                esp = esp.wrapping_add(0x20);
                push!(0);
                push!(0x361710);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0xb5e);
                call!(0x3d7b0);
                pc = 0x322cce; continue;
            }
            0x322cce => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x322cd7; continue;
            }
            0x322cd7 => {
                call!(0x781a0);
                pc = 0x322cdc; continue;
            }
            0x322cdc => {
                call!(0x22b90);
                pc = 0x322ce1; continue;
            }
            0x322ce1 => {
                xmm0 = Xmm::scalar(m.read32(0x7ba188));
                xmm2 = xmm0;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x475d24))).to_bits();
                xmm1.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d30))).to_bits();
                eax = m.read32(0x7bb1e8);
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x7ba184))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x7ba030))).to_bits();
                esp = esp.wrapping_add(0x14);
                m.write32(esp, xmm1.lanes[0]);
                push!(eax);
                call!(0x77320);
                pc = 0x322d2b; continue;
            }
            0x322d2b => {
                esp = esp.wrapping_add(0x8);
                call!(0x75e70);
                pc = 0x322d33; continue;
            }
            0x322d33 => {
                eax = m.read32(esp.wrapping_add(0x4));
                m.write32(eax.wrapping_add(0x218), (m.read32(eax.wrapping_add(0x218)) | 0x4));
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}
}
pub use unformatted::*;
