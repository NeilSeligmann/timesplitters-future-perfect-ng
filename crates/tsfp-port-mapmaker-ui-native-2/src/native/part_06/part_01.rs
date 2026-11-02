use super::*; // portreg: replaces=tsfp-ports/mapmaker_ui_lane
#[rustfmt::skip] mod unformatted { use super::*;

/// 0x0032DBA0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f32dba0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x32dba0;
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
        { crate::pose::cover(0x32dba0, previous, pc); previous = pc; }
        match pc {
            0x32dba0 => {
                esp = esp.wrapping_sub(0x3c);
                push!(ebp);
                push!(0xffffffffu32);
                call!(0x19650);
                pc = 0x32dbab; continue;
            }
            0x32dbab => {
                ecx = m.read32(0x778d34);
                m.write32(esp.wrapping_add(0x1c), eax);
                eax = m.read32(ecx);
                ebp = 0;
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ebp;
                if (fa == fb) { pc = 0x32e7dc; continue; }
                pc = 0x32dbc4; continue;
            }
            0x32dbc4 => {
                eax = m.read32(ecx.wrapping_add(0xc));
                edx = if ((((eax) as i32) < 0i32)) { 0xffffffffu32 } else { 0 };
                eax = eax.wrapping_sub(edx);
                edx = m.read32(ecx.wrapping_add(0x4));
                push!(esi);
                eax = (((((eax as i32)).wrapping_shr((((((((0x1i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                esi = edx;
                esi = esi.wrapping_sub(eax);
                esi = esi.wrapping_add(1);
                edx = edx.wrapping_add(eax);
                fa = esi;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x32e7db; continue; }
                pc = 0x32dbdf; continue;
            }
            0x32dbdf => {
                eax = m.read32(ecx.wrapping_add(0x10));
                ecx = m.read32(ecx.wrapping_add(0x8));
                edx = if ((((eax) as i32) < 0i32)) { 0xffffffffu32 } else { 0 };
                eax = eax.wrapping_sub(edx);
                eax = (((((eax as i32)).wrapping_shr((((((((0x1i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                edx = ecx;
                edx = edx.wrapping_sub(eax);
                edx = edx.wrapping_add(1);
                ecx = ecx.wrapping_add(eax);
                fa = edx;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x32e7db; continue; }
                pc = 0x32dbf9; continue;
            }
            0x32dbf9 => {
                push!(ebx);
                push!(edi);
                call!(0x55ea0);
                pc = 0x32dc00; continue;
            }
            0x32dc00 => {
                esi = m.read32(0x778d34);
                eax = m.read32(esi.wrapping_add(0x10));
                ecx = m.read32(esi.wrapping_add(0x8));
                edx = if ((((eax) as i32) < 0i32)) { 0xffffffffu32 } else { 0 };
                eax = eax.wrapping_sub(edx);
                edi = eax;
                eax = m.read32(esi.wrapping_add(0xc));
                edx = if ((((eax) as i32) < 0i32)) { 0xffffffffu32 } else { 0 };
                eax = eax.wrapping_sub(edx);
                edx = m.read32(esi.wrapping_add(0x4));
                edi = (((((edi as i32)).wrapping_shr((((((((0x1i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                esi = ecx.wrapping_add(edi);
                eax = (((((eax as i32)).wrapping_shr((((((((0x1i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                push!(esi);
                esi = edx.wrapping_add(eax);
                ecx = ecx.wrapping_sub(edi);
                ecx = ecx.wrapping_add(1);
                push!(esi);
                edx = edx.wrapping_sub(eax);
                push!(ecx);
                edx = edx.wrapping_add(1);
                push!(edx);
                call!(0x56050);
                pc = 0x32dc33; continue;
            }
            0x32dc33 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x25c));
                esp = esp.wrapping_add(0x10);
                fa = ecx;
                fb = ((((0x2000000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) { pc = 0x32dc85; continue; }
                pc = 0x32dc49; continue;
            }
            0x32dc49 => {
                esi = m.read32(0x778d34);
                eax = m.read32(esi.wrapping_add(0x20));
                ecx = m.read32(esi.wrapping_add(0x18));
                edx = if ((((eax) as i32) < 0i32)) { 0xffffffffu32 } else { 0 };
                eax = eax.wrapping_sub(edx);
                edi = eax;
                eax = m.read32(esi.wrapping_add(0x1c));
                edx = if ((((eax) as i32) < 0i32)) { 0xffffffffu32 } else { 0 };
                eax = eax.wrapping_sub(edx);
                edx = m.read32(esi.wrapping_add(0x14));
                push!(0x78787878);
                edi = (((((edi as i32)).wrapping_shr((((((((0x1i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                esi = ecx.wrapping_add(edi);
                push!(0x78);
                eax = (((((eax as i32)).wrapping_shr((((((((0x1i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                push!(esi);
                esi = edx.wrapping_add(eax);
                ecx = ecx.wrapping_sub(edi);
                push!(esi);
                ecx = ecx.wrapping_add(1);
                push!(ecx);
                edx = edx.wrapping_sub(eax);
                push!(edx);
                call!(0x81ec0);
                pc = 0x32dc82; continue;
            }
            0x32dc82 => {
                esp = esp.wrapping_add(0x18);
                pc = 0x32dc85; continue;
            }
            0x32dc85 => {
                esi = m.read32(0x778d34);
                eax = m.read32(esi.wrapping_add(0x20));
                ecx = m.read32(esi.wrapping_add(0x18));
                edx = if ((((eax) as i32) < 0i32)) { 0xffffffffu32 } else { 0 };
                eax = eax.wrapping_sub(edx);
                edi = eax;
                eax = m.read32(esi.wrapping_add(0x1c));
                edx = if ((((eax) as i32) < 0i32)) { 0xffffffffu32 } else { 0 };
                eax = eax.wrapping_sub(edx);
                edx = m.read32(esi.wrapping_add(0x14));
                edi = (((((edi as i32)).wrapping_shr((((((((0x1i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                esi = ecx.wrapping_add(edi);
                eax = (((((eax as i32)).wrapping_shr((((((((0x1i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                push!(esi);
                esi = edx.wrapping_add(eax);
                ecx = ecx.wrapping_sub(edi);
                ecx = ecx.wrapping_add(1);
                push!(esi);
                edx = edx.wrapping_sub(eax);
                push!(ecx);
                edx = edx.wrapping_add(1);
                push!(edx);
                call!(0x1a010);
                pc = 0x32dcb8; continue;
            }
            0x32dcb8 => {
                xmm0 = Xmm::scalar(m.read32(0x4d1920));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x4760a8))).to_bits();
                xmm1 = Xmm::default();
                esp = esp.wrapping_add(0x10);
                push!(0x43800000);
                push!(ecx);
                m.write32(esp.wrapping_add(0x1c), xmm0.lanes[0]);
                ecx = m.read32(esp.wrapping_add(0x1c));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp, xmm1.lanes[0]);
                push!(ecx);
                call!(0x55e70);
                pc = 0x32dced; continue;
            }
            0x32dced => {
                esp = esp.wrapping_add(0xc);
                call!(0x56080);
                pc = 0x32dcf5; continue;
            }
            0x32dcf5 => {
                eax = m.read32(0x778d34);
                fa = m.read32(eax.wrapping_add(0x94));
                fb = ebp;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x32e04e; continue; }
                pc = 0x32dd06; continue;
            }
            0x32dd06 => {
                xmm0 = Xmm::scalar(m.read32(0x475cd4));
                m.write32(esp.wrapping_add(0x28), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x47ddec));
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x475d4c));
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x49f844));
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                m.write32(esp.wrapping_add(0x14), xmm0.lanes[0]);
                xmm0.lanes[0] = reciprocal_sqrt(f32::from_bits(m.read32(esp.wrapping_add(0x10)))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x14)))).to_bits();
                m.write32(esp.wrapping_add(0x18), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x18)));
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x28)))).to_bits();
                m.write32(esp.wrapping_add(0x28), xmm1.lanes[0]);
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x2c)))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x30)))).to_bits();
                push!(0x4);
                m.write32(esp.wrapping_add(0x30), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x34), xmm0.lanes[0]);
                call!(0x19650);
                pc = 0x32dd95; continue;
            }
            0x32dd95 => {
                push!(ebp);
                call!(0x19d70);
                pc = 0x32dd9b; continue;
            }
            0x32dd9b => {
                eax = m.read32(0x778d34);
                ecx = m.read32(eax.wrapping_add(0x1e5a8));
                esp = esp.wrapping_add(0x8);
                fa = ecx;
                fb = ebp;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x10), ebp);
                if (fas <= fbs) { pc = 0x32decf; continue; }
                pc = 0x32ddb5; continue;
            }
            0x32ddb5 => {
                edi = 0;
                pc = 0x32ddc0; continue;
            }
            0x32ddc0 => {
                fa = m.read32(edi.wrapping_add(eax).wrapping_add(0x5cc));
                fb = ((((0x7fi32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x32deb3; continue; }
                pc = 0x32ddce; continue;
            }
            0x32ddce => {
                edx = m.read32(edi.wrapping_add(eax).wrapping_add(0x5d8));
                ecx = m.read32(edx.wrapping_add(0x10));
                ecx = ecx.wrapping_add(ecx.wrapping_mul(0x4));
                ecx = ecx.wrapping_shl((((((((0x6i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = ((m.read8(ecx.wrapping_add(0x529520))) as u8 as u32);
                ecx = m.read32(0x7844a8);
                esi = m.read32(ecx.wrapping_add(edx.wrapping_mul(0x4)).wrapping_add(0x40d8));
                edx = m.read32(edi.wrapping_add(eax).wrapping_add(0x5a8));
                m.write32(esi.wrapping_add(0x64), edx);
                eax = m.read32(0x778d34);
                ecx = m.read32(edi.wrapping_add(eax).wrapping_add(0x5ac));
                m.write32(esi.wrapping_add(0x68), ecx);
                edx = m.read32(0x778d34);
                eax = m.read32(edi.wrapping_add(edx).wrapping_add(0x5b0));
                m.write32(esi.wrapping_add(0x6c), eax);
                ecx = m.read32(0x778d34);
                eax = m.read32(esi.wrapping_add(0x4));
                edx = edi.wrapping_add(ecx).wrapping_add(0x5dc);
                ecx = m.read32(eax.wrapping_add(0x10));
                push!(edx);
                push!(ecx);
                call!(0x1531d0);
                pc = 0x32de35; continue;
            }
            0x32de35 => {
                edx = m.read32(0x778d34);
                eax = m.read32(esi.wrapping_add(0x4));
                ebx = m.read32(eax.wrapping_add(0x30));
                ecx = m.read32(edi.wrapping_add(edx).wrapping_add(0x5d4));
                ebp = (ebp & 0xffff_0000) | ((((m.read16(esi.wrapping_add(0x14))) as u32)) & 0xffff);
                m.write32(eax.wrapping_add(0x30), ecx);
                m.write32(esi.wrapping_add(0x28), (m.read32(esi.wrapping_add(0x28)) | 0x200008));
                edx = m.read32(0x778d34);
                eax = m.read32(edi.wrapping_add(edx).wrapping_add(0x5d8));
                eax = eax.wrapping_add(0x18);
                push!(eax);
                call!(0x30a2d0);
                pc = 0x32de6c; continue;
            }
            0x32de6c => {
                push!(eax);
                call!(0x32d9b0);
                pc = 0x32de72; continue;
            }
            0x32de72 => {
                ecx = m.read32(0x778d34);
                edx = m.read32(edi.wrapping_add(ecx).wrapping_add(0x5cc));
                ecx = m.read32(esi.wrapping_add(0x4));
                push!(eax);
                push!(edx);
                eax = esp.wrapping_add(0x40);
                push!(eax);
                push!(ecx);
                call!(0x945c0);
                pc = 0x32de8f; continue;
            }
            0x32de8f => {
                xmm0 = Xmm::default();
                edx = m.read32(esi.wrapping_add(0x4));
                m.write32(esi.wrapping_add(0x64), xmm0.lanes[0]);
                m.write32(esi.wrapping_add(0x68), xmm0.lanes[0]);
                m.write32(esi.wrapping_add(0x6c), xmm0.lanes[0]);
                m.write32(edx.wrapping_add(0x30), ebx);
                write16(m, esi.wrapping_add(0x14), (((((ebp) as u16)) as u32)) as u16);
                eax = m.read32(0x778d34);
                esp = esp.wrapping_add(0x20);
                pc = 0x32deb3; continue;
            }
            0x32deb3 => {
                ecx = m.read32(esp.wrapping_add(0x10));
                edx = m.read32(eax.wrapping_add(0x1e5a8));
                ecx = ecx.wrapping_add(1);
                edi = edi.wrapping_add(0x78);
                fa = ecx;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x10), ecx);
                if (fas < fbs) { pc = 0x32ddc0; continue; }
                pc = 0x32decd; continue;
            }
            0x32decd => {
                ebp = 0;
                pc = 0x32decf; continue;
            }
            0x32decf => {
                fa = m.read32(eax.wrapping_add(0x1e5a8));
                fb = ebp;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x10), ebp);
                if (fas <= fbs) { pc = 0x32e038; continue; }
                pc = 0x32dedf; continue;
            }
            0x32dedf => {
                edi = 0;
                pc = 0x32def0; continue;
            }
            0x32def0 => {
                ecx = m.read32(edi.wrapping_add(eax).wrapping_add(0x5cc));
                fa = ecx;
                fb = ((((0x19i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x32e01c; continue; }
                pc = 0x32df00; continue;
            }
            0x32df00 => {
                fa = ecx;
                fb = ((((0x7fi32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x32e01c; continue; }
                pc = 0x32df09; continue;
            }
            0x32df09 => {
                ecx = m.read32(edi.wrapping_add(eax).wrapping_add(0x5d8));
                ecx = m.read32(ecx.wrapping_add(0x10));
                eax = m.read32(edi.wrapping_add(eax).wrapping_add(0x5a8));
                edx = ecx.wrapping_add(ecx.wrapping_mul(0x4));
                edx = edx.wrapping_shl((((((((0x6i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = ((m.read8(edx.wrapping_add(0x529520))) as u8 as u32);
                edx = m.read32(0x7844a8);
                esi = m.read32(edx.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x40d8));
                m.write32(esi.wrapping_add(0x64), eax);
                ecx = m.read32(0x778d34);
                edx = m.read32(edi.wrapping_add(ecx).wrapping_add(0x5ac));
                m.write32(esi.wrapping_add(0x68), edx);
                eax = m.read32(0x778d34);
                ecx = m.read32(edi.wrapping_add(eax).wrapping_add(0x5b0));
                m.write32(esi.wrapping_add(0x6c), ecx);
                edx = m.read32(0x778d34);
                ecx = m.read32(esi.wrapping_add(0x4));
                eax = edi.wrapping_add(edx).wrapping_add(0x5dc);
                edx = m.read32(ecx.wrapping_add(0x10));
                push!(eax);
                push!(edx);
                call!(0x1531d0);
                pc = 0x32df70; continue;
            }
            0x32df70 => {
                ecx = m.read32(0x778d34);
                eax = m.read32(esi.wrapping_add(0x4));
                ebx = m.read32(eax.wrapping_add(0x30));
                edx = m.read32(edi.wrapping_add(ecx).wrapping_add(0x5d4));
                ebp = (ebp & 0xffff_0000) | ((((m.read16(esi.wrapping_add(0x14))) as u32)) & 0xffff);
                m.write32(eax.wrapping_add(0x30), edx);
                m.write32(esi.wrapping_add(0x28), (m.read32(esi.wrapping_add(0x28)) | 0x200008));
                eax = m.read32(0x778d34);
                ecx = m.read32(edi.wrapping_add(eax).wrapping_add(0x5d8));
                ecx = ecx.wrapping_add(0x18);
                push!(ecx);
                call!(0x30a2d0);
                pc = 0x32dfa6; continue;
            }
            0x32dfa6 => {
                edx = eax;
                edx = edx.wrapping_shr((((((((0x18i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = eax;
                ecx = ecx.wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = edx.wrapping_shl((((((((0x19i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = ecx.wrapping_shl((((((((0x11i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx & 0xff0000);
                edx = (edx & 0xff000000u32);
                edx = (edx | ecx);
                ecx = eax;
                ecx = ecx.wrapping_shr((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = ecx.wrapping_shl((((((((0x9i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx & 0xff00);
                edx = (edx | ecx);
                eax = (eax & 0xff);
                edx = (edx | eax);
                push!(edx);
                edx = m.read32(0x778d34);
                eax = m.read32(edi.wrapping_add(edx).wrapping_add(0x5cc));
                edx = m.read32(esi.wrapping_add(0x4));
                push!(eax);
                ecx = esp.wrapping_add(0x3c);
                push!(ecx);
                push!(edx);
                call!(0x945c0);
                pc = 0x32dff8; continue;
            }
            0x32dff8 => {
                xmm0 = Xmm::default();
                eax = m.read32(esi.wrapping_add(0x4));
                m.write32(esi.wrapping_add(0x64), xmm0.lanes[0]);
                m.write32(esi.wrapping_add(0x68), xmm0.lanes[0]);
                m.write32(esi.wrapping_add(0x6c), xmm0.lanes[0]);
                m.write32(eax.wrapping_add(0x30), ebx);
                write16(m, esi.wrapping_add(0x14), (((((ebp) as u16)) as u32)) as u16);
                eax = m.read32(0x778d34);
                esp = esp.wrapping_add(0x1c);
                pc = 0x32e01c; continue;
            }
            0x32e01c => {
                ecx = m.read32(esp.wrapping_add(0x10));
                edx = m.read32(eax.wrapping_add(0x1e5a8));
                ecx = ecx.wrapping_add(1);
                edi = edi.wrapping_add(0x78);
                fa = ecx;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x10), ecx);
                if (fas < fbs) { pc = 0x32def0; continue; }
                pc = 0x32e036; continue;
            }
            0x32e036 => {
                ebp = 0;
                pc = 0x32e038; continue;
            }
            0x32e038 => {
                push!(0x1);
                call!(0x19d70);
                pc = 0x32e03f; continue;
            }
            0x32e03f => {
                push!(0xa);
                call!(0x19650);
                pc = 0x32e046; continue;
            }
            0x32e046 => {
                eax = m.read32(0x778d34);
                esp = esp.wrapping_add(0x8);
                pc = 0x32e04e; continue;
            }
            0x32e04e => {
                ecx = m.read32(eax.wrapping_add(0x90));
                fa = ecx;
                fb = ebp;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x32e21b; continue; }
                pc = 0x32e05c; continue;
            }
            0x32e05c => {
                fa = ((((m.read8(eax.wrapping_add(0x80)))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x32e09d; continue; }
                pc = 0x32e065; continue;
            }
            0x32e065 => {
                eax = m.read32(0x7844a8);
                ecx = ecx.wrapping_add(ecx.wrapping_mul(0x4));
                ecx = ecx.wrapping_shl((((((((0x6i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = ((m.read8(ecx.wrapping_add(0x529520))) as u8 as u32);
                eax = m.read32(eax.wrapping_add(edx.wrapping_mul(0x4)).wrapping_add(0x4064));
                edi = m.read32(eax.wrapping_add(0x28));
                ecx = m.read32(eax.wrapping_add(0x4));
                push!(0x7f);
                edi = (edi | 0x200008);
                push!(ecx);
                m.write32(eax.wrapping_add(0x28), edi);
                call!(0x94570);
                pc = 0x32e095; continue;
            }
            0x32e095 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x32e205; continue;
            }
            0x32e09d => {
                xmm0 = Xmm::scalar(m.read32(0x475cd4));
                ecx = m.read32(0x7844a8);
                m.write32(esp.wrapping_add(0x28), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x47ddec));
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x475d4c));
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                eax = m.read32(eax.wrapping_add(0x90));
                edx = eax.wrapping_add(eax.wrapping_mul(0x4));
                edx = edx.wrapping_shl((((((((0x6i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = ((m.read8(edx.wrapping_add(0x529520))) as u8 as u32);
                esi = m.read32(ecx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x40d8));
                m.write32(esi.wrapping_add(0x28), (m.read32(esi.wrapping_add(0x28)) | 0x200008));
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x2f7c));
                fa = eax;
                fb = ebp;
                if (fa == fb) { pc = 0x32e145; continue; }
                pc = 0x32e0fe; continue;
            }
            0x32e0fe => {
                eax = eax.wrapping_add(0x18);
                push!(eax);
                call!(0x30a2d0);
                pc = 0x32e107; continue;
            }
            0x32e107 => {
                ecx = eax;
                edx = ecx;
                edx = edx.wrapping_shr((((((((0x18i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = edx.wrapping_shl((((((((0x19i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = edx;
                edx = ecx;
                edx = edx.wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = edx.wrapping_shl((((((((0x11i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = (edx & 0xff0000);
                eax = (eax & 0xff000000u32);
                eax = (eax | edx);
                edx = ecx;
                edx = edx.wrapping_shr((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = edx.wrapping_shl((((((((0x9i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = (edx & 0xff00);
                eax = (eax | edx);
                ecx = (ecx & 0xff);
                esp = esp.wrapping_add(0x4);
                eax = (eax | ecx);
                pc = 0x32e166; continue;
            }
            0x32e145 => {
                ecx = m.read32(ecx.wrapping_add(0x2f18));
                fa = ecx;
                fb = ebp;
                if (fa == fb) { pc = 0x32e163; continue; }
                pc = 0x32e14f; continue;
            }
            0x32e14f => {
                ecx = ecx.wrapping_add(0x18);
                push!(ecx);
                call!(0x30a2d0);
                pc = 0x32e158; continue;
            }
            0x32e158 => {
                push!(eax);
                call!(0x32d9b0);
                pc = 0x32e15e; continue;
            }
            0x32e15e => {
                esp = esp.wrapping_add(0x8);
                pc = 0x32e166; continue;
            }
            0x32e163 => {
                eax = (eax | 0xffffffffu32);
                pc = 0x32e166; continue;
            }
            0x32e166 => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x2c)));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x30)));
                xmm2 = xmm1;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x28)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = xmm1;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x4765f4)) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x32e1f1; continue; }
                pc = 0x32e19b; continue;
            }
            0x32e19b => {
                m.write32(esp.wrapping_add(0x14), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                m.write32(esp.wrapping_add(0x18), xmm0.lanes[0]);
                xmm0.lanes[0] = reciprocal_sqrt(f32::from_bits(m.read32(esp.wrapping_add(0x14)))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x18)))).to_bits();
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x10)));
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x28)))).to_bits();
                m.write32(esp.wrapping_add(0x28), xmm1.lanes[0]);
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x2c)))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x30)))).to_bits();
                m.write32(esp.wrapping_add(0x2c), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                pc = 0x32e1f1; continue;
            }
            0x32e1f1 => {
                ecx = m.read32(esi.wrapping_add(0x4));
                push!(eax);
                push!(0x7f);
                eax = esp.wrapping_add(0x30);
                push!(eax);
                push!(ecx);
                call!(0x945c0);
                pc = 0x32e202; continue;
            }
            0x32e202 => {
                esp = esp.wrapping_add(0x10);
                pc = 0x32e205; continue;
            }
            0x32e205 => {
                push!(0x1);
                call!(0x19d70);
                pc = 0x32e20c; continue;
            }
            0x32e20c => {
                push!(0xa);
                call!(0x19650);
                pc = 0x32e213; continue;
            }
            0x32e213 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x32e4e9; continue;
            }
            0x32e21b => {
                ecx = m.read32(eax.wrapping_add(0x88));
                fa = ecx;
                fb = ebp;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x32e4ee; continue; }
                pc = 0x32e229; continue;
            }
            0x32e229 => {
                fa = m.read32(eax.wrapping_add(0x84));
                fb = ebp;
                if (fa == fb) { pc = 0x32e4ee; continue; }
                pc = 0x32e235; continue;
            }
            0x32e235 => {
                eax = m.read32(0x7844a8);
                edi = m.read32(eax.wrapping_add(0x4150));
                fa = ((((m.read16(edi.wrapping_add(0x94)))) as u32) & 0xffffu32);
                fb = ((((0x7a1i32)) as u32) & 0xffffu32);
                if (fa != fb) { pc = 0x32e495; continue; }
                pc = 0x32e24f; continue;
            }
            0x32e24f => {
                fa = m.read32(eax.wrapping_add(0x254));
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x32e495; continue; }
                pc = 0x32e25c; continue;
            }
            0x32e25c => {
                xmm2 = Xmm::default();
                xmm3.lanes[0] = (((m.read32(0x4e8cec)) as i32) as f32).to_bits();
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                edx = edi;
                ecx = m.read32(edx.wrapping_add(0x7c));
                m.write32(esp.wrapping_add(0x10), ecx);
                xmm1 = Xmm::scalar(m.read32(edx.wrapping_add(0x14c)));
                xmm0.lanes[0] = sse::divss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x4e794c))).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x28), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0x2c), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0x30), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0x14), xmm1.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x32e2f5; continue; }
                pc = 0x32e2ad; continue;
            }
            0x32e2ad => {
                call!(0x153b40);
                pc = 0x32e2b2; continue;
            }
            0x32e2b2 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x20), eax);
                fpu.push((m.read32(esp.wrapping_add(0x20)) as i32 as f64));
                if (fas >= fbs) { pc = 0x32e2c4; continue; }
                pc = 0x32e2be; continue;
            }
            0x32e2be => {
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475ccc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                pc = 0x32e2c4; continue;
            }
            0x32e2c4 => {
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4785d0)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                eax = m.read32(esp.wrapping_add(0x10));
                fa = eax;
                fb = ebp;
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x475d04)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x4785e0)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x14), (fpu.st(0) as f32).to_bits()); fpu.pop();
                if (fa == fb) { pc = 0x32e320; continue; }
                pc = 0x32e2e2; continue;
            }
            0x32e2e2 => {
                push!(ebp);
                push!(eax);
                call!(0xff420);
                pc = 0x32e2e9; continue;
            }
            0x32e2e9 => {
                ecx = m.read32(esp.wrapping_add(0x18));
                eax = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x8);
                pc = 0x32e2f5; continue;
            }
            0x32e2f5 => {
                fa = ecx;
                fb = ebp;
                if (fa == fb) { pc = 0x32e320; continue; }
                pc = 0x32e2f9; continue;
            }
            0x32e2f9 => {
                xmm0 = Xmm::scalar(m.read32(0x475d1c));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(ecx.wrapping_add(0x30))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x32e471; continue; }
                pc = 0x32e30b; continue;
            }
            0x32e30b => {
                push!(ebp);
                push!(ecx);
                call!(0xff420);
                pc = 0x32e312; continue;
            }
            0x32e312 => {
                eax = m.read32(esp.wrapping_add(0x18));
                m.write32(eax.wrapping_add(0x14), ebp);
                esp = esp.wrapping_add(0x8);
                m.write32(esp.wrapping_add(0x10), ebp);
                pc = 0x32e320; continue;
            }
            0x32e320 => {
                xmm0 = Xmm::scalar(m.read32(0x47d698));
                m.write32(esp.wrapping_add(0x40), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x49f840));
                m.write32(esp.wrapping_add(0x44), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x47dac0));
                m.write32(esp.wrapping_add(0x48), xmm0.lanes[0]);
                call!(0x153b40);
                pc = 0x32e34f; continue;
            }
            0x32e34f => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x20), eax);
                fpu.push((m.read32(esp.wrapping_add(0x20)) as i32 as f64));
                if (fas >= fbs) { pc = 0x32e361; continue; }
                pc = 0x32e35b; continue;
            }
            0x32e35b => {
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475ccc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                pc = 0x32e361; continue;
            }
            0x32e361 => {
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4785d0)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(0x475cd4)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x475cdc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x34), (fpu.st(0) as f32).to_bits()); fpu.pop();
                call!(0x153b40);
                pc = 0x32e37c; continue;
            }
            0x32e37c => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x20), eax);
                fpu.push((m.read32(esp.wrapping_add(0x20)) as i32 as f64));
                if (fas >= fbs) { pc = 0x32e38e; continue; }
                pc = 0x32e388; continue;
            }
            0x32e388 => {
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475ccc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                pc = 0x32e38e; continue;
            }
            0x32e38e => {
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4785d0)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(0x475cd4)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x475cdc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x38), (fpu.st(0) as f32).to_bits()); fpu.pop();
                call!(0x153b40);
                pc = 0x32e3a9; continue;
            }
            0x32e3a9 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x20), eax);
                fpu.push((m.read32(esp.wrapping_add(0x20)) as i32 as f64));
                if (fas >= fbs) { pc = 0x32e3bb; continue; }
                pc = 0x32e3b5; continue;
            }
            0x32e3b5 => {
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475ccc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                pc = 0x32e3bb; continue;
            }
            0x32e3bb => {
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4785d0)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                ecx = m.read32(0x778d34);
                xmm0 = Xmm::scalar(m.read32(0x4788fc));
                push!(0xff7fffffu32);
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(0x475cd4)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(ebp);
                edx = esp.wrapping_add(0x30);
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x475cdc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(edx);
                eax = esp.wrapping_add(0x40);
                push!(eax);
                m.write32(esp.wrapping_add(0x4c), (fpu.st(0) as f32).to_bits()); fpu.pop();
                m.write32(ecx.wrapping_add(0x40), xmm0.lanes[0]);
                ecx = esp.wrapping_add(0x50);
                push!(ecx);
                push!(0x20000000);
                edx = esp.wrapping_add(0x28);
                push!(0x5f);
                push!(edx);
                call!(0x1006d0);
                pc = 0x32e40a; continue;
            }
            0x32e40a => {
                push!(0x41000000);
                push!(eax);
                m.write32(esp.wrapping_add(0x38), eax);
                call!(0xff420);
                pc = 0x32e419; continue;
            }
            0x32e419 => {
                eax = m.read32(0x778d34);
                eax = m.read32(eax.wrapping_add(0x88));
                eax = ((eax as i32).wrapping_mul((0xc4) as i32)) as u32;
                edx = ((m.read8(eax.wrapping_add(0x524f5d))) as u8 as u32);
                ecx = m.read32(eax.wrapping_add(0x524f5c));
                ecx = ecx.wrapping_shl((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx | edx);
                edx = ((m.read8(eax.wrapping_add(0x524f5e))) as u8 as u32);
                eax = ((m.read8(eax.wrapping_add(0x524f5f))) as u8 as u32);
                ecx = ecx.wrapping_shl((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx | edx);
                edx = m.read32(esp.wrapping_add(0x38));
                ecx = ecx.wrapping_shl((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx | eax);
                eax = m.read32(edx.wrapping_add(0x18));
                m.write32(eax.wrapping_add(0xfa4), ecx);
                ecx = m.read32(esp.wrapping_add(0x38));
                esp = esp.wrapping_add(0x28);
                fa = ecx;
                fb = ebp;
                if (fa == fb) { pc = 0x32e47a; continue; }
                pc = 0x32e46c; continue;
            }
            0x32e46c => {
                eax = m.read32(0x7844a8);
                pc = 0x32e471; continue;
            }
            0x32e471 => {
                edx = m.read32(eax.wrapping_add(0x4150));
                m.write32(edx.wrapping_add(0x7c), ecx);
                pc = 0x32e47a; continue;
            }
            0x32e47a => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x4150));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                m.write32(ecx.wrapping_add(0x14c), xmm0.lanes[0]);
                pc = 0x32e4e9; continue;
            }
            0x32e495 => {
                fa = ecx;
                fb = ((((0x46i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x32e4c0; continue; }
                pc = 0x32e49a; continue;
            }
            0x32e49a => {
                fa = ecx;
                fb = ((((0x47i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x32e4c0; continue; }
                pc = 0x32e49f; continue;
            }
            0x32e49f => {
                push!(0x4);
                call!(0x19650);
                pc = 0x32e4a6; continue;
            }
            0x32e4a6 => {
                edx = m.read32(0x7844a8);
                eax = m.read32(edx.wrapping_add(0x4150));
                ecx = m.read32(eax.wrapping_add(0x4));
                push!(ecx);
                call!(0x96810);
                pc = 0x32e4bb; continue;
            }
            0x32e4bb => {
                esp = esp.wrapping_add(0x8);
                pc = 0x32e4e9; continue;
            }
            0x32e4c0 => {
                esi = m.read32(eax.wrapping_add(0x4154));
                push!(0x4);
                call!(0x19650);
                pc = 0x32e4cd; continue;
            }
            0x32e4cd => {
                edx = m.read32(edi.wrapping_add(0x4));
                push!(edx);
                call!(0x96810);
                pc = 0x32e4d6; continue;
            }
            0x32e4d6 => {
                push!(0x4);
                call!(0x19650);
                pc = 0x32e4dd; continue;
            }
            0x32e4dd => {
                eax = m.read32(esi.wrapping_add(0x4));
                push!(eax);
                call!(0x96810);
                pc = 0x32e4e6; continue;
            }
            0x32e4e6 => {
                esp = esp.wrapping_add(0x10);
                pc = 0x32e4e9; continue;
            }
            0x32e4e9 => {
                eax = m.read32(0x778d34);
                pc = 0x32e4ee; continue;
            }
            0x32e4ee => {
                fa = ((((m.read8(eax.wrapping_add(0x80)))) as u32) & 0xffu32);
                fb = ((((0x40i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x32e77d; continue; }
                pc = 0x32e4fb; continue;
            }
            0x32e4fb => {
                ecx = m.read32(eax.wrapping_add(0x90));
                fa = ecx;
                fb = ebp;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x32e5eb; continue; }
                pc = 0x32e509; continue;
            }
            0x32e509 => {
                esi = ecx.wrapping_add(ecx.wrapping_mul(0x4));
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x200));
                esi = esi.wrapping_shl((((((((0x6i32)) as u32) & 0x1fu32))) as u32 & 31));
                ebx = 0;
                esi = esi.wrapping_add(0x529510);
                fa = eax;
                fb = ebp;
                push!(0x4);
                ebx = (ebx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                call!(0x3fa10);
                pc = 0x32e52f; continue;
            }
            0x32e52f => {
                edx = m.read32(0x7ba9e0);
                edx = ((edx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                xmm0 = Xmm::scalar(m.read32(edx.wrapping_add(0x7ba344)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x4760cc))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x7ba038))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x7ba180))).to_bits();
                esp = esp.wrapping_add(0x4);
                edi = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                call!(0x22b90);
                pc = 0x32e567; continue;
            }
            0x32e567 => {
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                xmm1 = Xmm::scalar(m.read32(0x7ba188));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x47d1e4))).to_bits();
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                ebp = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                xmm0 = Xmm::scalar(m.read32(0x7ba038));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x47d1dc))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x7ba180))).to_bits();
                xmm1.lanes[0] = (((edi) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                eax = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                if (fa == fb) { pc = 0x32e5b1; continue; }
                pc = 0x32e5ab; continue;
            }
            0x32e5ab => {
                esi = ((m.read8(esi.wrapping_add(0x4))) as u8 as u32);
                pc = 0x32e5b4; continue;
            }
            0x32e5b1 => {
                esi = (esi | 0xffffffffu32);
                pc = 0x32e5b4; continue;
            }
            0x32e5b4 => {
                push!(0x1);
                push!(eax);
                eax = m.read32(0x778d34);
                eax = m.read32(eax.wrapping_add(0x90));
                ecx = eax.wrapping_add(eax.wrapping_mul(0x4));
                ecx = ecx.wrapping_shl((((((((0x6i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = m.read32(ecx.wrapping_add(0x529518));
                push!(edx);
                call!(0x3d7b0);
                pc = 0x32e5d4; continue;
            }
            0x32e5d4 => {
                esp = esp.wrapping_add(0x4);
                ecx = eax;
                push!(edi);
                eax = esi;
                edx = ebp;
                call!(0x32d9f0);
                pc = 0x32e5e3; continue;
            }
            0x32e5e3 => {
                esp = esp.wrapping_add(0xc);
                pc = 0x32e778; continue;
            }
            0x32e5eb => {
                ecx = m.read32(eax.wrapping_add(0x88));
                fa = ecx;
                fb = ebp;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x32e77d; continue; }
                pc = 0x32e5f9; continue;
            }
            0x32e5f9 => {
                push!(ecx);
                call!(0x308340);
                pc = 0x32e5ff; continue;
            }
            0x32e5ff => {
                edx = m.read32(0x7844a8);
                edi = m.read32(edx.wrapping_add(0x200));
                m.write32(esp.wrapping_add(0x20), eax);
                eax = m.read32(0x778d34);
                esi = m.read32(eax.wrapping_add(0x88));
                esi = ((esi as i32).wrapping_mul((0xc4) as i32)) as u32;
                ecx = 0;
                esi = esi.wrapping_add(0x524f58);
                eax = (((((m.read16(esi) as i16)) as i32)) as u32);
                fa = edi;
                fb = ebp;
                ecx = (ecx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                push!(eax);
                m.write32(esp.wrapping_add(0x20), ecx);
                call!(0x3d7b0);
                pc = 0x32e63a; continue;
            }
            0x32e63a => {
                ecx = m.read32(0x778d34);
                edx = m.read32(ecx.wrapping_add(0x88));
                push!(edx);
                m.write32(esp.wrapping_add(0x1c), eax);
                call!(0x315e10);
                pc = 0x32e650; continue;
            }
            0x32e650 => {
                m.write32(esp.wrapping_add(0x20), eax);
                eax = m.read32(esi.wrapping_add(0xb8));
                esp = esp.wrapping_add(0xc);
                fa = (((((((eax) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x32e678; continue; }
                pc = 0x32e662; continue;
            }
            0x32e662 => {
                eax = m.read32(0x778d34);
                ecx = m.read32(eax.wrapping_add(0x88));
                push!(ecx);
                call!(0x303350);
                pc = 0x32e673; continue;
            }
            0x32e673 => {
                esp = esp.wrapping_add(0x4);
                ebp = eax;
                pc = 0x32e678; continue;
            }
            0x32e678 => {
                push!(0x4);
                call!(0x3fa10);
                pc = 0x32e67f; continue;
            }
            0x32e67f => {
                edx = m.read32(0x7ba9e0);
                xmm0 = Xmm::scalar(m.read32(0x7ba038));
                edx = ((edx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                xmm2 = Xmm::scalar(m.read32(edx.wrapping_add(0x7ba344)));
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x4760cc))).to_bits();
                xmm1 = Xmm::scalar(m.read32(0x7ba180));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x47d1dc))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                edi = sse::cvttss2si(f32::from_bits(xmm2.lanes[0]));
                xmm2.lanes[0] = (((edi) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                esp = esp.wrapping_add(0x4);
                m.write32(esp.wrapping_add(0x20), xmm2.lanes[0]);
                ebx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                call!(0x22b90);
                pc = 0x32e6dd; continue;
            }
            0x32e6dd => {
                xmm1 = Xmm::scalar(m.read32(0x7ba188));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x47d1e4))).to_bits();
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                eax = m.read32(esp.wrapping_add(0x18));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                esi = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                if (fa == fb) { pc = 0x32e707; continue; }
                pc = 0x32e701; continue;
            }
            0x32e701 => {
                eax = m.read32(esp.wrapping_add(0x14));
                pc = 0x32e70a; continue;
            }
            0x32e707 => {
                eax = (eax | 0xffffffffu32);
                pc = 0x32e70a; continue;
            }
            0x32e70a => {
                ecx = m.read32(esp.wrapping_add(0x10));
                push!(0x1);
                push!(ebx);
                push!(edi);
                edx = esi;
                call!(0x32d9f0);
                pc = 0x32e719; continue;
            }
            0x32e719 => {
                push!(0x12);
                esi = esi.wrapping_add(eax);
                call!(0x3fa10);
                pc = 0x32e722; continue;
            }
            0x32e722 => {
                esp = esp.wrapping_add(0x10);
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x32e73e; continue; }
                pc = 0x32e729; continue;
            }
            0x32e729 => {
                push!(0);
                push!(ebx);
                push!(edi);
                ecx = ebp;
                eax = (eax | 0xffffffffu32);
                edx = esi;
                call!(0x32d9f0);
                pc = 0x32e739; continue;
            }
            0x32e739 => {
                esp = esp.wrapping_add(0xc);
                esi = esi.wrapping_add(eax);
                pc = 0x32e73e; continue;
            }
            0x32e73e => {
                eax = m.read32(esp.wrapping_add(0x1c));
                push!(0x40);
                push!(0x5050507f);
                push!(eax);
                push!(0xd78);
                call!(0x3d7b0);
                pc = 0x32e754; continue;
            }
            0x32e754 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x151030);
                pc = 0x32e75d; continue;
            }
            0x32e75d => {
                esp = esp.wrapping_add(0x8);
                push!(eax);
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x30));
                xmm0.lanes[0] = (((esi) as i32) as f32).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                push!(ecx);
                call!(0x41e30);
                pc = 0x32e775; continue;
            }
            0x32e775 => {
                esp = esp.wrapping_add(0x14);
                pc = 0x32e778; continue;
            }
            0x32e778 => {
                eax = m.read32(0x778d34);
                pc = 0x32e77d; continue;
            }
            0x32e77d => {
                ecx = m.read32(eax.wrapping_add(0x90));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x32e79c; continue; }
                pc = 0x32e787; continue;
            }
            0x32e787 => {
                fa = ((((m.read8(eax.wrapping_add(0x80)))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x32e79c; continue; }
                pc = 0x32e790; continue;
            }
            0x32e790 => {
                ebx = m.read32(ecx.wrapping_mul(0x4).wrapping_add(0x783f40));
                call!(0x32ce00);
                pc = 0x32e79c; continue;
            }
            0x32e79c => {
                push!(0x1);
                call!(0x19d70);
                pc = 0x32e7a3; continue;
            }
            0x32e7a3 => {
                edx = m.read32(esp.wrapping_add(0x28));
                push!(edx);
                call!(0x19650);
                pc = 0x32e7ad; continue;
            }
            0x32e7ad => {
                esp = esp.wrapping_add(0x8);
                call!(0x55ec0);
                pc = 0x32e7b5; continue;
            }
            0x32e7b5 => {
                call!(0x56080);
                pc = 0x32e7ba; continue;
            }
            0x32e7ba => {
                eax = m.read32(0x7844a8);
                eax = m.read32(eax.wrapping_add(0x4150));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                if (fa == fb) { pc = 0x32e7db; continue; }
                pc = 0x32e7cb; continue;
            }
            0x32e7cb => {
                fa = ((((m.read16(eax.wrapping_add(0x94)))) as u32) & 0xffffu32);
                fb = ((((0x7a1i32)) as u32) & 0xffffu32);
                if (fa != fb) { pc = 0x32e7db; continue; }
                pc = 0x32e7d6; continue;
            }
            0x32e7d6 => {
                call!(0x10adf0);
                pc = 0x32e7db; continue;
            }
            0x32e7db => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x32e7dc; continue;
            }
            0x32e7dc => {
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x3c);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}
/// 0x00331C70: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f331c70<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x331c70;
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
        { crate::pose::cover(0x331c70, previous, pc); previous = pc; }
        match pc {
            0x331c70 => {
                eax = 0xffff8008u32;
                fa = ((((m.read16(esp.wrapping_add(0x4)))) as u32) & 0xffffu32);
                fb = ((((((eax) as u16))) as u32) & 0xffffu32);
                if (fa == fb) { pc = 0x331c95; continue; }
                pc = 0x331c7c; continue;
            }
            0x331c7c => {
                ecx = m.read32(esp.wrapping_add(0x4));
                ecx = ecx.wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                fa = ((((((ecx) as u16))) as u32) & 0xffffu32);
                fb = ((((((eax) as u16))) as u32) & 0xffffu32);
                if (fa == fb) { pc = 0x331c95; continue; }
                pc = 0x331c88; continue;
            }
            0x331c88 => {
                fa = ((((m.read16(esp.wrapping_add(0x8)))) as u32) & 0xffffu32);
                fb = ((((((eax) as u16))) as u32) & 0xffffu32);
                if (fa == fb) { pc = 0x331c95; continue; }
                pc = 0x331c8f; continue;
            }
            0x331c8f => {
                eax = 0x1;
                break 'run;
            }
            0x331c95 => {
                eax = 0;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00334730: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f334730<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x334730;
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
        { crate::pose::cover(0x334730, previous, pc); previous = pc; }
        match pc {
            0x334730 => {
                ecx = m.read32(esp.wrapping_add(0x8));
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                eax = m.read32(esi.wrapping_add(0x10));
                call!(0x3319c0);
                pc = 0x334741; continue;
            }
            0x334741 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x33474c; continue; }
                pc = 0x334745; continue;
            }
            0x334745 => {
                eax = 0x1;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x33474c => {
                push!(esi);
                call!(0x2fc470);
                pc = 0x334752; continue;
            }
            0x334752 => {
                push!(eax);
                eax = m.read32(esi.wrapping_add(0x10));
                push!(eax);
                call!(0x331920);
                pc = 0x33475c; continue;
            }
            0x33475c => {
                esp = esp.wrapping_add(0xc);
                cf = (((((eax) != 0))) as i32);
                eax = (((((eax) as i32).wrapping_neg())) as u32);
                eax = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                eax = eax.wrapping_add(1);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x003368B0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3368b0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3368b0;
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
        { crate::pose::cover(0x3368b0, previous, pc); previous = pc; }
        match pc {
            0x3368b0 => {
                call!(0x336860);
                pc = 0x3368b5; continue;
            }
            0x3368b5 => {
                push!(eax);
                push!(0xd46);
                call!(0x3d7b0);
                pc = 0x3368c0; continue;
            }
            0x3368c0 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                eax = m.read32(esp.wrapping_add(0xc));
                push!(0x1ff);
                push!(eax);
                call!(0x3ca0b5);
                pc = 0x3368d3; continue;
            }
            0x3368d3 => {
                esp = esp.wrapping_add(0x10);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x003368E0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3368e0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3368e0;
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
        { crate::pose::cover(0x3368e0, previous, pc); previous = pc; }
        match pc {
            0x3368e0 => {
                call!(0x336860);
                pc = 0x3368e5; continue;
            }
            0x3368e5 => {
                push!(eax);
                push!(0xd48);
                call!(0x3d7b0);
                pc = 0x3368f0; continue;
            }
            0x3368f0 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                eax = m.read32(esp.wrapping_add(0xc));
                push!(0x1ff);
                push!(eax);
                call!(0x3ca0b5);
                pc = 0x336903; continue;
            }
            0x336903 => {
                esp = esp.wrapping_add(0x10);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00336910: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f336910<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x336910;
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
        { crate::pose::cover(0x336910, previous, pc); previous = pc; }
        match pc {
            0x336910 => {
                eax = m.read32(0x7844a8);
                eax = eax.wrapping_add(0x2f34);
                push!(eax);
                call!(0x303430);
                pc = 0x336920; continue;
            }
            0x336920 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0xd4a);
                call!(0x3d7b0);
                pc = 0x33692e; continue;
            }
            0x33692e => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                eax = m.read32(esp.wrapping_add(0xc));
                push!(0x1ff);
                push!(eax);
                call!(0x3ca0b5);
                pc = 0x336941; continue;
            }
            0x336941 => {
                esp = esp.wrapping_add(0x10);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00336950: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f336950<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x336950;
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
        { crate::pose::cover(0x336950, previous, pc); previous = pc; }
        match pc {
            0x336950 => {
                call!(0x336860);
                pc = 0x336955; continue;
            }
            0x336955 => {
                push!(eax);
                push!(0xd4f);
                call!(0x3d7b0);
                pc = 0x336960; continue;
            }
            0x336960 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                eax = m.read32(esp.wrapping_add(0xc));
                push!(0x1ff);
                push!(eax);
                call!(0x3ca0b5);
                pc = 0x336973; continue;
            }
            0x336973 => {
                esp = esp.wrapping_add(0x10);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00336D30: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f336d30<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x336d30;
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
        { crate::pose::cover(0x336d30, previous, pc); previous = pc; }
        match pc {
            0x336d30 => {
                eax = m.read32(0x7844a8);
                ecx = (((((m.read16(eax.wrapping_add(0x2f12)) as i16)) as i32)) as u32);
                edx = m.read32(esp.wrapping_add(0x4));
                eax = (((((m.read16(eax.wrapping_add(0x2f10)) as i16)) as i32)) as u32);
                push!(0xd4a);
                push!(ecx);
                push!(edx);
                call!(0x336c50);
                pc = 0x336d53; continue;
            }
            0x336d53 => {
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00336D60: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f336d60<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x336d60;
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
        { crate::pose::cover(0x336d60, previous, pc); previous = pc; }
        match pc {
            0x336d60 => {
                eax = m.read32(0x7844a8);
                eax = m.read32(eax.wrapping_add(0x2f20));
                edx = m.read32(esp.wrapping_add(0x4));
                ecx = eax;
                push!(0xd46);
                ecx = (ecx & 0xffff);
                push!(ecx);
                eax = (((((eax as i32)).wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                push!(edx);
                call!(0x336c50);
                pc = 0x336d86; continue;
            }
            0x336d86 => {
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00336D90: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f336d90<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x336d90;
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
        { crate::pose::cover(0x336d90, previous, pc); previous = pc; }
        match pc {
            0x336d90 => {
                eax = m.read32(0x7844a8);
                eax = m.read32(eax.wrapping_add(0x2f20));
                edx = m.read32(esp.wrapping_add(0x4));
                ecx = eax;
                push!(0xd48);
                ecx = (ecx & 0xffff);
                push!(ecx);
                eax = (((((eax as i32)).wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                push!(edx);
                call!(0x336c50);
                pc = 0x336db6; continue;
            }
            0x336db6 => {
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00336DC0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f336dc0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x336dc0;
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
        { crate::pose::cover(0x336dc0, previous, pc); previous = pc; }
        match pc {
            0x336dc0 => {
                eax = m.read32(0x7844a8);
                eax = m.read32(eax.wrapping_add(0x2f20));
                edx = m.read32(esp.wrapping_add(0x4));
                ecx = eax;
                push!(0xd4f);
                ecx = (ecx & 0xffff);
                push!(ecx);
                eax = (((((eax as i32)).wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                push!(edx);
                call!(0x336c50);
                pc = 0x336de6; continue;
            }
            0x336de6 => {
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00336E60: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f336e60<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x336e60;
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
        { crate::pose::cover(0x336e60, previous, pc); previous = pc; }
        match pc {
            0x336e60 => {
                call!(0x31ba80);
                pc = 0x336e65; continue;
            }
            0x336e65 => {
                push!(eax);
                call!(0x31baa0);
                pc = 0x336e6b; continue;
            }
            0x336e6b => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0xd4f);
                call!(0x3d7b0);
                pc = 0x336e79; continue;
            }
            0x336e79 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                eax = m.read32(esp.wrapping_add(0xc));
                push!(0x1ff);
                push!(eax);
                call!(0x3ca0b5);
                pc = 0x336e8c; continue;
            }
            0x336e8c => {
                esp = esp.wrapping_add(0x10);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00336F60: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f336f60<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x336f60;
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
        { crate::pose::cover(0x336f60, previous, pc); previous = pc; }
        match pc {
            0x336f60 => {
                esp = esp.wrapping_sub(0x8);
                eax = esp;
                push!(eax);
                ecx = esp.wrapping_add(0x8);
                push!(ecx);
                push!(0x1);
                call!(0x31fd60);
                pc = 0x336f73; continue;
            }
            0x336f73 => {
                edx = m.read32(esp.wrapping_add(0xc));
                edx = edx.wrapping_add(0x2);
                push!(edx);
                call!(0x31dc10);
                pc = 0x336f80; continue;
            }
            0x336f80 => {
                push!(eax);
                call!(0x3d7b0);
                pc = 0x336f86; continue;
            }
            0x336f86 => {
                esp = esp.wrapping_add(0x14);
                push!(eax);
                push!(0xd4c);
                call!(0x3d7b0);
                pc = 0x336f94; continue;
            }
            0x336f94 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x14));
                push!(0x1ff);
                push!(eax);
                call!(0x3ca0b5);
                pc = 0x336fa7; continue;
            }
            0x336fa7 => {
                esp = esp.wrapping_add(0x18);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00336FB0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f336fb0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x336fb0;
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
        { crate::pose::cover(0x336fb0, previous, pc); previous = pc; }
        match pc {
            0x336fb0 => {
                esp = esp.wrapping_sub(0x8);
                eax = esp;
                push!(eax);
                ecx = esp.wrapping_add(0x8);
                push!(ecx);
                push!(0x2);
                call!(0x31fd60);
                pc = 0x336fc3; continue;
            }
            0x336fc3 => {
                edx = m.read32(esp.wrapping_add(0xc));
                edx = edx.wrapping_add(0x2);
                push!(edx);
                call!(0x31dc10);
                pc = 0x336fd0; continue;
            }
            0x336fd0 => {
                push!(eax);
                call!(0x3d7b0);
                pc = 0x336fd6; continue;
            }
            0x336fd6 => {
                esp = esp.wrapping_add(0x14);
                push!(eax);
                push!(0xd46);
                call!(0x3d7b0);
                pc = 0x336fe4; continue;
            }
            0x336fe4 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x14));
                push!(0x1ff);
                push!(eax);
                call!(0x3ca0b5);
                pc = 0x336ff7; continue;
            }
            0x336ff7 => {
                esp = esp.wrapping_add(0x18);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00337ED0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f337ed0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x337ed0;
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
        { crate::pose::cover(0x337ed0, previous, pc); previous = pc; }
        match pc {
            0x337ed0 => {
                push!(esi);
                push!(edi);
                push!(0);
                push!(0);
                push!(0x425c0000);
                push!(0);
                push!(0x8800);
                call!(0x75b70);
                pc = 0x337ee7; continue;
            }
            0x337ee7 => {
                push!(0x9b4);
                call!(0x3d7b0);
                pc = 0x337ef1; continue;
            }
            0x337ef1 => {
                ecx = m.read32(esp.wrapping_add(0x24));
                push!(0x1);
                push!(0x1);
                push!(0);
                push!(0x1);
                m.write32(ecx.wrapping_add(0x204), eax);
                m.write32(ecx.wrapping_add(0x218), 0x5);
                call!(0x71b10);
                pc = 0x337f12; continue;
            }
            0x337f12 => {
                push!(0x20000);
                call!(0x71c50);
                pc = 0x337f1c; continue;
            }
            0x337f1c => {
                esp = esp.wrapping_add(0x2c);
                push!(0);
                push!(0);
                push!(0);
                call!(0x22b90);
                pc = 0x337f2a; continue;
            }
            0x337f2a => {
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
                pc = 0x337f78; continue;
            }
            0x337f78 => {
                push!(0x40000000);
                call!(0x784a0);
                pc = 0x337f82; continue;
            }
            0x337f82 => {
                esp = esp.wrapping_add(0x14);
                edi = 0;
                esi = 0x52ebb8;
                pc = 0x337f90; continue;
            }
            0x337f90 => {
                eax = m.read32(esi);
                push!(0);
                push!(0x361710);
                push!(0);
                push!(0x800);
                push!(edi);
                push!(eax);
                call!(0x3d7b0);
                pc = 0x337fa7; continue;
            }
            0x337fa7 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x337fb0; continue;
            }
            0x337fb0 => {
                esp = esp.wrapping_add(0x18);
                esi = esi.wrapping_add(0x14);
                edi = edi.wrapping_add(1);
                fa = esi;
                fb = ((((0x52ec80i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x337f90; continue; }
                pc = 0x337fbf; continue;
            }
            0x337fbf => {
                call!(0x781a0);
                pc = 0x337fc4; continue;
            }
            0x337fc4 => {
                call!(0x75e70);
                pc = 0x337fc9; continue;
            }
            0x337fc9 => {
                call!(0x22b90);
                pc = 0x337fce; continue;
            }
            0x337fce => {
                xmm0 = Xmm::scalar(m.read32(0x7ba188));
                xmm2 = xmm0;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x475d24))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d30))).to_bits();
                xmm1.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x7ba184))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x7ba030))).to_bits();
                push!(ecx);
                ecx = m.read32(0x7bb1e8);
                m.write32(esp, xmm1.lanes[0]);
                push!(ecx);
                call!(0x77320);
                pc = 0x338017; continue;
            }
            0x338017 => {
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
}
pub use unformatted::*;
