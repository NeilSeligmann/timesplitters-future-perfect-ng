use super::*; // portreg: replaces=tsfp-ports/mapmaker_ui_lane
#[rustfmt::skip] mod unformatted { use super::*;

/// 0x0030CF60: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f30cf60<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x30cf60;
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
        { crate::pose::cover(0x30cf60, previous, pc); previous = pc; }
        match pc {
            0x30cf60 => {
                ecx = m.read32(0x7ba9e0);
                eax = ((m.read8(0x7ba034)) as u8 as u32);
                ecx = ((ecx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                xmm0 = Xmm::scalar(m.read32(ecx.wrapping_add(0x7ba278)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x7ba038))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x7ba180))).to_bits();
                push!(ebx);
                push!(esi);
                esi = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                xmm0 = Xmm::scalar(m.read32(ecx.wrapping_add(0x7ba27c)));
                ecx = m.read32(0x784010);
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x7ba188))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x7ba184))).to_bits();
                edx = eax;
                edx = (edx | 0x7f7f7f00);
                fa = ecx;
                fb = ((((0x10i32)) as u32) & 0xffffffffu32);
                push!(edi);
                edi = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                if (fa > fb) { pc = 0x30d008; continue; }
                pc = 0x30cfc1; continue;
            }
            0x30cfc1 => {
                ebx = ((m.read8(ecx.wrapping_add(0x30d108))) as u8 as u32);
                jt = m.read32(ebx.wrapping_mul(0x4).wrapping_add(0x30d0e8));
                if (jt == 0x30cfcfu32) { pc = 0x30cfcf; continue; }
                if (jt == 0x30cfd7u32) { pc = 0x30cfd7; continue; }
                if (jt == 0x30cfe6u32) { pc = 0x30cfe6; continue; }
                if (jt == 0x30cfeeu32) { pc = 0x30cfee; continue; }
                if (jt == 0x30cff6u32) { pc = 0x30cff6; continue; }
                if (jt == 0x30cffcu32) { pc = 0x30cffc; continue; }
                if (jt == 0x30d005u32) { pc = 0x30d005; continue; }
                if (jt == 0x30d008u32) { pc = 0x30d008; continue; }
                tail_to!(jt); break 'run;  // indirect tail jump, target outside the switch
            }
            0x30cfcf => {
                esi = esi.wrapping_sub(0x8);
                edi = edi.wrapping_sub(0x6);
                pc = 0x30d008; continue;
            }
            0x30cfd7 => {
                edi = edi.wrapping_sub(0x3);
                esi = esi.wrapping_sub(0x6);
                eax = (eax | 0x7f3f0000);
                edx = eax;
                pc = 0x30d008; continue;
            }
            0x30cfe6 => {
                edi = edi.wrapping_sub(0x4);
                esi = esi.wrapping_sub(0x8);
                pc = 0x30d008; continue;
            }
            0x30cfee => {
                esi = esi.wrapping_sub(0x5);
                edi = edi.wrapping_sub(0x3);
                pc = 0x30d008; continue;
            }
            0x30cff6 => {
                esi = esi.wrapping_sub(0x6);
                edi = edi.wrapping_sub(0x3);
                pc = 0x30cffc; continue;
            }
            0x30cffc => {
                eax = (eax | 0x7f7f00);
                edx = eax;
                pc = 0x30d008; continue;
            }
            0x30d005 => {
                esi = esi.wrapping_sub(0x2);
                pc = 0x30d008; continue;
            }
            0x30d008 => {
                eax = m.read32(0x7e71c8);
                push!(edx);
                edx = m.read32(eax);
                push!(ecx);
                ecx = m.read32(eax.wrapping_add(0x14));
                ecx = ecx.wrapping_add(edi);
                esp = esp.wrapping_sub(0x8);
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                edx = edx.wrapping_add(esi);
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                call!(0x81c60);
                pc = 0x30d033; continue;
            }
            0x30d033 => {
                eax = m.read32(0x784010);
                esp = esp.wrapping_add(0x10);
                fa = eax;
                fb = ((((0xfi32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x30d07c; continue; }
                pc = 0x30d040; continue;
            }
            0x30d040 => {
                push!(0x783a60);
                call!(0x30a2d0);
                pc = 0x30d04a; continue;
            }
            0x30d04a => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                eax = m.read32(0x7e71c8);
                ecx = m.read32(eax.wrapping_add(0x14));
                edx = m.read32(eax);
                push!(0x32);
                ecx = ecx.wrapping_add(edi);
                esp = esp.wrapping_sub(0x8);
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                edx = edx.wrapping_add(esi);
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                call!(0x81c60);
                pc = 0x30d079; continue;
            }
            0x30d079 => {
                esp = esp.wrapping_add(0x10);
                pc = 0x30d07c; continue;
            }
            0x30d07c => {
                eax = m.read32(0x7844a8);
                fa = ((((m.read8(eax.wrapping_add(0x11854)))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x30d0e2; continue; }
                pc = 0x30d08a; continue;
            }
            0x30d08a => {
                push!(0x80);
                push!(0x64646480);
                push!(0xa98);
                call!(0x3d7b0);
                pc = 0x30d09e; continue;
            }
            0x30d09e => {
                xmm1 = Xmm::scalar(m.read32(0x7ba038));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x475d10))).to_bits();
                esp = esp.wrapping_add(0x4);
                push!(eax);
                eax = m.read32(0x7e71c8);
                ecx = m.read32(eax.wrapping_add(0x14));
                edx = m.read32(eax);
                ecx = ecx.wrapping_add(edi);
                esp = esp.wrapping_sub(0x8);
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                edx = edx.wrapping_add(esi);
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                call!(0x41e30);
                pc = 0x30d0df; continue;
            }
            0x30d0df => {
                esp = esp.wrapping_add(0x14);
                pc = 0x30d0e2; continue;
            }
            0x30d0e2 => {
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

/// 0x0030D120: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f30d120<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x30d120;
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
        { crate::pose::cover(0x30d120, previous, pc); previous = pc; }
        match pc {
            0x30d120 => {
                xmm1 = Xmm::default();
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x4)));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30d2f5; continue; }
                pc = 0x30d136; continue;
            }
            0x30d136 => {
                xmm0.lanes[0] = (((m.read32(0x4e8cf4)) as i32) as f32).to_bits();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esi.wrapping_add(0x4))) as f64);
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30d2f5; continue; }
                pc = 0x30d148; continue;
            }
            0x30d148 => {
                xmm2 = Xmm::scalar(m.read32(esi.wrapping_add(0xc)));
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30d2f5; continue; }
                pc = 0x30d156; continue;
            }
            0x30d156 => {
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm2.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30d2f5; continue; }
                pc = 0x30d15f; continue;
            }
            0x30d15f => {
                push!(ebx);
                push!(edi);
                push!(0);
                call!(0x80f20);
                pc = 0x30d168; continue;
            }
            0x30d168 => {
                push!(eax);
                ebx = 0x4;
                push!(ebx);
                push!(0x3);
                push!(0x92);
                push!(0x2);
                call!(0x1cd90);
                pc = 0x30d17d; continue;
            }
            0x30d17d => {
                eax = m.read32(esi.wrapping_add(0x4));
                xmm0 = Xmm::scalar(m.read32(esi));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x2c)))).to_bits();
                esp = esp.wrapping_add(0x18);
                push!(0);
                push!(eax);
                push!(ecx);
                m.write32(esp, xmm0.lanes[0]);
                call!(0xf9080);
                pc = 0x30d19b; continue;
            }
            0x30d19b => {
                eax = m.read32(esp.wrapping_add(0x24));
                ecx = m.read32(0x55f974);
                xmm0 = Xmm::default();
                edi = eax;
                eax = eax.wrapping_shr((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                edi = edi.wrapping_shl((((((((0x18i32)) as u32) & 0x1fu32))) as u32 & 31));
                edi = (edi | eax);
                m.write32(ecx, edi);
                eax = m.read32(0x55f974);
                eax = eax.wrapping_add(ebx);
                m.write32(0x55f974, eax);
                m.write32(eax, xmm0.lanes[0]);
                eax = m.read32(0x55f974);
                eax = eax.wrapping_add(ebx);
                m.write32(0x55f974, eax);
                m.write32(eax, xmm0.lanes[0]);
                eax = m.read32(0x55f974);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x20)));
                esp = esp.wrapping_add(0xc);
                eax = eax.wrapping_add(ebx);
                m.write32(0x55f974, eax);
                edx = m.read32(esi.wrapping_add(0x4));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x8)))).to_bits();
                push!(0);
                push!(edx);
                push!(ecx);
                m.write32(esp, xmm0.lanes[0]);
                call!(0xf9080);
                pc = 0x30d1ff; continue;
            }
            0x30d1ff => {
                eax = m.read32(0x55f974);
                xmm0 = Xmm::default();
                m.write32(eax, edi);
                eax = m.read32(0x55f974);
                eax = eax.wrapping_add(ebx);
                m.write32(0x55f974, eax);
                m.write32(eax, xmm0.lanes[0]);
                eax = m.read32(0x55f974);
                eax = eax.wrapping_add(ebx);
                m.write32(0x55f974, eax);
                m.write32(eax, xmm0.lanes[0]);
                edx = m.read32(0x55f974);
                esp = esp.wrapping_add(0xc);
                edx = edx.wrapping_add(ebx);
                m.write32(0x55f974, edx);
                ecx = m.read32(esi.wrapping_add(0xc));
                xmm0 = Xmm::scalar(m.read32(esi));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x14)))).to_bits();
                push!(0);
                push!(ecx);
                push!(ecx);
                m.write32(esp, xmm0.lanes[0]);
                call!(0xf9080);
                pc = 0x30d255; continue;
            }
            0x30d255 => {
                eax = m.read32(esp.wrapping_add(0x28));
                edx = m.read32(0x55f974);
                edi = eax;
                eax = eax.wrapping_shr((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                edi = edi.wrapping_shl((((((((0x18i32)) as u32) & 0x1fu32))) as u32 & 31));
                edi = (edi | eax);
                m.write32(edx, edi);
                eax = m.read32(0x55f974);
                esp = esp.wrapping_add(0xc);
                eax = eax.wrapping_add(ebx);
                xmm0 = Xmm::default();
                m.write32(0x55f974, eax);
                m.write32(eax, xmm0.lanes[0]);
                eax = m.read32(0x55f974);
                eax = eax.wrapping_add(ebx);
                m.write32(0x55f974, eax);
                m.write32(eax, xmm0.lanes[0]);
                m.write32(0x55f974, m.read32(0x55f974).wrapping_add(ebx));
                eax = m.read32(esi.wrapping_add(0xc));
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x8)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x14)))).to_bits();
                push!(0);
                push!(eax);
                push!(ecx);
                m.write32(esp, xmm0.lanes[0]);
                call!(0xf9080);
                pc = 0x30d2b3; continue;
            }
            0x30d2b3 => {
                ecx = m.read32(0x55f974);
                xmm0 = Xmm::default();
                m.write32(ecx, edi);
                eax = m.read32(0x55f974);
                eax = eax.wrapping_add(ebx);
                m.write32(0x55f974, eax);
                m.write32(eax, xmm0.lanes[0]);
                eax = m.read32(0x55f974);
                eax = eax.wrapping_add(ebx);
                m.write32(0x55f974, eax);
                esp = esp.wrapping_add(0xc);
                m.write32(eax, xmm0.lanes[0]);
                eax = m.read32(0x55f974);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = eax.wrapping_add(ebx);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(0x55f974, eax);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                tail_to!(0x1d380); break 'run;  // direct tail jump
            }
            0x30d2f5 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0030D300: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f30d300<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x30d300;
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
        { crate::pose::cover(0x30d300, previous, pc); previous = pc; }
        match pc {
            0x30d300 => {
                xmm1 = Xmm::default();
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x4)));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30d400; continue; }
                pc = 0x30d316; continue;
            }
            0x30d316 => {
                xmm0.lanes[0] = (((m.read32(0x4e8cf4)) as i32) as f32).to_bits();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esi.wrapping_add(0x4))) as f64);
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30d400; continue; }
                pc = 0x30d328; continue;
            }
            0x30d328 => {
                xmm2 = Xmm::scalar(m.read32(esi.wrapping_add(0xc)));
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30d400; continue; }
                pc = 0x30d336; continue;
            }
            0x30d336 => {
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm2.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30d400; continue; }
                pc = 0x30d33f; continue;
            }
            0x30d33f => {
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                xmm3 = Xmm::scalar(m.read32(esi));
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x18));
                xmm1 = xmm2;
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                eax = sse::cvttss2si(f32::from_bits(xmm1.lanes[0]));
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x10)));
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                push!(edi);
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                push!(eax);
                ecx = sse::cvttss2si(f32::from_bits(xmm3.lanes[0]));
                xmm3 = Xmm::scalar(m.read32(esi.wrapping_add(0x4)));
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::scalar(m.read32(esi));
                push!(ecx);
                edx = sse::cvttss2si(f32::from_bits(xmm3.lanes[0]));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                push!(edx);
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                eax = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(eax);
                call!(0x1b090);
                pc = 0x30d39a; continue;
            }
            0x30d39a => {
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                xmm1 = Xmm::scalar(m.read32(esi.wrapping_add(0xc)));
                xmm3 = Xmm::scalar(m.read32(esi.wrapping_add(0x8)));
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x28)));
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                ecx = sse::cvttss2si(f32::from_bits(xmm1.lanes[0]));
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                edx = sse::cvttss2si(f32::from_bits(xmm3.lanes[0]));
                xmm3 = Xmm::scalar(m.read32(esi.wrapping_add(0x4)));
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                push!(edi);
                eax = sse::cvttss2si(f32::from_bits(xmm3.lanes[0]));
                xmm3 = Xmm::scalar(m.read32(esi.wrapping_add(0x8)));
                push!(ecx);
                push!(edx);
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                push!(eax);
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                ecx = sse::cvttss2si(f32::from_bits(xmm3.lanes[0]));
                push!(ecx);
                call!(0x1b090);
                pc = 0x30d3fc; continue;
            }
            0x30d3fc => {
                esp = esp.wrapping_add(0x28);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x30d400; continue;
            }
            0x30d400 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}
}
pub use unformatted::*;
