use super::*; // portreg: replaces=tsfp-ports/mapmaker_ui_lane
#[rustfmt::skip] mod unformatted { use super::*;
/// 0x00310A20: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f310a20<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x310a20;
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
        { crate::pose::cover(0x310a20, previous, pc); previous = pc; }
        match pc {
            0x310a20 => {
                esp = esp.wrapping_sub(0x14);
                ecx = m.read32(0x7844a8);
                push!(ebx);
                ebx = m.read32(ecx.wrapping_add(0x254));
                eax = 0;
                fa = ebx;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                push!(ebp);
                eax = (eax & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                push!(esi);
                push!(edi);
                push!(0);
                m.write32(esp.wrapping_add(0x20), eax);
                call!(0x19d70);
                pc = 0x310a46; continue;
            }
            0x310a46 => {
                call!(0x30a550);
                pc = 0x310a4b; continue;
            }
            0x310a4b => {
                push!(0x1);
                call!(0x19d70);
                pc = 0x310a52; continue;
            }
            0x310a52 => {
                esp = esp.wrapping_add(0x8);
                call!(0x310100);
                pc = 0x310a5a; continue;
            }
            0x310a5a => {
                call!(0x310310);
                pc = 0x310a5f; continue;
            }
            0x310a5f => {
                ecx = m.read32(0x7844a8);
                fa = m.read32(ecx.wrapping_add(0x25c));
                fb = ((((0x44088i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) != 0) { pc = 0x3111e4; continue; }
                pc = 0x310a75; continue;
            }
            0x310a75 => {
                fa = m.read32(ecx.wrapping_add(0x254));
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x310a8e; continue; }
                pc = 0x310a7e; continue;
            }
            0x310a7e => {
                push!(0);
                call!(0x309a80);
                pc = 0x310a85; continue;
            }
            0x310a85 => {
                ecx = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x4);
                pc = 0x310a8e; continue;
            }
            0x310a8e => {
                eax = m.read32(ecx.wrapping_add(0x254));
                fa = eax;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x310ab4; continue; }
                pc = 0x310a99; continue;
            }
            0x310a99 => {
                fa = eax;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x310ab4; continue; }
                pc = 0x310a9e; continue;
            }
            0x310a9e => {
                fa = eax;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x310d58; continue; }
                pc = 0x310aa7; continue;
            }
            0x310aa7 => {
                fa = m.read32(ecx.wrapping_add(0x1185c));
                fb = ((((0x6i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x310d58; continue; }
                pc = 0x310ab4; continue;
            }
            0x310ab4 => {
                eax = m.read32(ecx.wrapping_add(0x2f18));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x310d58; continue; }
                pc = 0x310ac2; continue;
            }
            0x310ac2 => {
                edx = (((((m.read16(ecx.wrapping_add(0x2f2e)) as i16)) as i32)) as u32);
                eax = (((((m.read16(ecx.wrapping_add(0x2f30)) as i16)) as i32)) as u32);
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                m.write32(esp.wrapping_add(0x18), xmm0.lanes[0]);
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                push!(0x2);
                m.write32(esp.wrapping_add(0x14), xmm0.lanes[0]);
                call!(0x19d70);
                pc = 0x310aeb; continue;
            }
            0x310aeb => {
                edi = eax;
                eax = m.read32(0x7844a8);
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0x25c))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x310b0f; continue; }
                pc = 0x310b00; continue;
            }
            0x310b00 => {
                esi = ((m.read8(0x7ba17c)) as u8 as u32);
                esi = (esi | 0xfe000001u32);
                pc = 0x310b1c; continue;
            }
            0x310b0f => {
                esi = ((m.read8(0x7ba034)) as u8 as u32);
                esi = (esi | 0xfe0000);
                pc = 0x310b1c; continue;
            }
            0x310b1c => {
                ecx = m.read32(eax.wrapping_add(0x254));
                ebx = m.read32(esp.wrapping_add(0x18));
                ebp = m.read32(esp.wrapping_add(0x10));
                esi = esi.wrapping_shr((((((((0x1i32)) as u32) & 0x1fu32))) as u32 & 31));
                fa = ecx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x310b57; continue; }
                pc = 0x310b31; continue;
            }
            0x310b31 => {
                ecx = m.read32(eax.wrapping_add(0x2f34));
                fa = ecx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x310b4e; continue; }
                pc = 0x310b3c; continue;
            }
            0x310b3c => {
                push!(ecx);
                call!(0x2f7da0);
                pc = 0x310b42; continue;
            }
            0x310b42 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x310ba5; continue; }
                pc = 0x310b49; continue;
            }
            0x310b49 => {
                eax = m.read32(0x7844a8);
                pc = 0x310b4e; continue;
            }
            0x310b4e => {
                fa = m.read32(eax.wrapping_add(0x2f78));
                fb = ((0xffffffffu32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x310ba5; continue; }
                pc = 0x310b57; continue;
            }
            0x310b57 => {
                fa = m.read32(eax.wrapping_add(0x254));
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x310b70; continue; }
                pc = 0x310b60; continue;
            }
            0x310b60 => {
                eax = m.read32(eax.wrapping_add(0x1185c));
                fa = eax;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x310ba5; continue; }
                pc = 0x310b6b; continue;
            }
            0x310b6b => {
                fa = eax;
                fb = ((((0x5i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x310ba5; continue; }
                pc = 0x310b70; continue;
            }
            0x310b70 => {
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x10)));
                push!(esi);
                esp = esp.wrapping_sub(0x8);
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x4), xmm1.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp, xmm1.lanes[0]);
                push!(ebp);
                push!(ebx);
                call!(0x308b10);
                pc = 0x310ba2; continue;
            }
            0x310ba2 => {
                esp = esp.wrapping_add(0x14);
                pc = 0x310ba5; continue;
            }
            0x310ba5 => {
                push!(edi);
                call!(0x19d70);
                pc = 0x310bab; continue;
            }
            0x310bab => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x254));
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x310bc8; continue; }
                pc = 0x310bbf; continue;
            }
            0x310bbf => {
                fa = eax;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x310cca; continue; }
                pc = 0x310bc8; continue;
            }
            0x310bc8 => {
                eax = m.read32(ecx.wrapping_add(0x2f34));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x310c77; continue; }
                pc = 0x310bd6; continue;
            }
            0x310bd6 => {
                push!(eax);
                call!(0x2f7da0);
                pc = 0x310bdc; continue;
            }
            0x310bdc => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x310c71; continue; }
                pc = 0x310be7; continue;
            }
            0x310be7 => {
                eax = m.read32(0x7844a8);
                esi = m.read32(eax.wrapping_add(0x2f34));
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0x25c))) as u32)) & 0xff);
                esi = ((esi as i32).wrapping_mul((0xc4) as i32)) as u32;
                edi = eax.wrapping_add(0x2f34);
                esi = esi.wrapping_add(0x524f58);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x310c1d; continue; }
                pc = 0x310c0f; continue;
            }
            0x310c0f => {
                eax = ((m.read8(0x7ba034)) as u8 as u32);
                eax = (eax | 0x407f4000);
                pc = 0x310c29; continue;
            }
            0x310c1d => {
                eax = ((m.read8(0x7ba17c)) as u8 as u32);
                eax = (eax | 0x7f404000);
                pc = 0x310c29; continue;
            }
            0x310c29 => {
                ecx = m.read32(edi.wrapping_add(0x4));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x18)));
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x10)));
                if (fas < fbs) { pc = 0x310c43; continue; }
                pc = 0x310c3c; continue;
            }
            0x310c3c => {
                push!(0x5064787f);
                pc = 0x310c45; continue;
            }
            0x310c43 => {
                push!(0);
                pc = 0x310c45; continue;
            }
            0x310c45 => {
                push!(edi);
                call!(0x30d7b0);
                pc = 0x310c4b; continue;
            }
            0x310c4b => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(0xb8))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x8);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x310cc4; continue; }
                pc = 0x310c58; continue;
            }
            0x310c58 => {
                ecx = m.read32(0x7844a8);
                fa = ((((m.read8(ecx.wrapping_add(0x25c)))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x310cca; continue; }
                pc = 0x310c67; continue;
            }
            0x310c67 => {
                edi = m.read32(edi.wrapping_add(0x8));
                eax = 0x1;
                pc = 0x310cba; continue;
            }
            0x310c71 => {
                ecx = m.read32(0x7844a8);
                pc = 0x310c77; continue;
            }
            0x310c77 => {
                eax = m.read32(ecx.wrapping_add(0x2f1c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x310cca; continue; }
                pc = 0x310c81; continue;
            }
            0x310c81 => {
                ecx = m.read32(eax);
                push!(ecx);
                call!(0x2f7da0);
                pc = 0x310c89; continue;
            }
            0x310c89 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x310cc4; continue; }
                pc = 0x310c90; continue;
            }
            0x310c90 => {
                ecx = m.read32(0x7844a8);
                edx = m.read32(ecx.wrapping_add(0x2f1c));
                eax = m.read32(edx);
                eax = ((eax as i32).wrapping_mul((0xc4) as i32)) as u32;
                fa = ((((m.read8(eax.wrapping_add(0x525010)))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x310cca; continue; }
                pc = 0x310cad; continue;
            }
            0x310cad => {
                eax = m.read32(esp.wrapping_add(0x1c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x310cca; continue; }
                pc = 0x310cb5; continue;
            }
            0x310cb5 => {
                edi = m.read32(edx.wrapping_add(0x8));
                eax = 0;
                pc = 0x310cba; continue;
            }
            0x310cba => {
                push!(ebp);
                push!(ebx);
                call!(0x30bc50);
                pc = 0x310cc1; continue;
            }
            0x310cc1 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x310cc4; continue;
            }
            0x310cc4 => {
                ecx = m.read32(0x7844a8);
                pc = 0x310cca; continue;
            }
            0x310cca => {
                fa = m.read32(ecx.wrapping_add(0x254));
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x310d58; continue; }
                pc = 0x310cd7; continue;
            }
            0x310cd7 => {
                eax = (eax & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(0x2f10))) as u32)) & 0xffff);
                fa = ((((((eax) as u16))) as u32) & 0xffffu32);
                fb = ((((0i32)) as u32) & 0xffffu32);
                fas = (((((fa)) as i16)) as i32);
                fbs = (((((fb)) as i16)) as i32);
                if (fas < fbs) { pc = 0x310d18; continue; }
                pc = 0x310ce3; continue;
            }
            0x310ce3 => {
                edx = ((((eax) as u16)) as u16 as i16 as i32 as u32);
                edx = ((edx as i32).wrapping_mul((0xe4) as i32)) as u32;
                eax = m.read32(edx.wrapping_add(ecx).wrapping_add(0x2bc));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                edx = edx.wrapping_add(ecx).wrapping_add(0x288);
                if (fa == fb) { pc = 0x310d08; continue; }
                pc = 0x310cfe; continue;
            }
            0x310cfe => {
                fa = eax;
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x310d08; continue; }
                pc = 0x310d03; continue;
            }
            0x310d03 => {
                fa = eax;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x310d58; continue; }
                pc = 0x310d08; continue;
            }
            0x310d08 => {
                fa = ((((m.read8(ecx.wrapping_add(0x25c)))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x310d58; continue; }
                pc = 0x310d11; continue;
            }
            0x310d11 => {
                eax = 0x1;
                pc = 0x310d45; continue;
            }
            0x310d18 => {
                eax = m.read32(ecx.wrapping_add(0x2f20));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x310d58; continue; }
                pc = 0x310d22; continue;
            }
            0x310d22 => {
                eax = (((((eax as i32)).wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                eax = ((eax as i32).wrapping_mul((0xe4) as i32)) as u32;
                edx = eax.wrapping_add(ecx).wrapping_add(0x288);
                eax = m.read32(edx.wrapping_add(0x34));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x310d43; continue; }
                pc = 0x310d39; continue;
            }
            0x310d39 => {
                fa = eax;
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x310d43; continue; }
                pc = 0x310d3e; continue;
            }
            0x310d3e => {
                fa = eax;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x310d58; continue; }
                pc = 0x310d43; continue;
            }
            0x310d43 => {
                eax = 0;
                pc = 0x310d45; continue;
            }
            0x310d45 => {
                edi = m.read32(edx.wrapping_add(0x54));
                push!(ebp);
                push!(ebx);
                call!(0x30bc50);
                pc = 0x310d4f; continue;
            }
            0x310d4f => {
                ecx = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x8);
                pc = 0x310d58; continue;
            }
            0x310d58 => {
                eax = m.read32(ecx.wrapping_add(0x254));
                fa = eax;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x310d6c; continue; }
                pc = 0x310d63; continue;
            }
            0x310d63 => {
                fa = eax;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x310f6b; continue; }
                pc = 0x310d6c; continue;
            }
            0x310d6c => {
                eax = m.read32(ecx.wrapping_add(0x2f34));
                push!(eax);
                call!(0x2f7da0);
                pc = 0x310d78; continue;
            }
            0x310d78 => {
                ecx = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x310f6b; continue; }
                pc = 0x310d89; continue;
            }
            0x310d89 => {
                eax = m.read32(ecx.wrapping_add(0x2f18));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x310f6b; continue; }
                pc = 0x310d97; continue;
            }
            0x310d97 => {
                esi = ((m.read8(0x7ba034)) as u8 as u32);
                edx = m.read32(eax.wrapping_add(0xc));
                eax = m.read32(eax.wrapping_add(edx.wrapping_mul(0x4)).wrapping_add(0x2c));
                edx = m.read32(eax.wrapping_add(0x44));
                esi = esi.wrapping_add(0x20);
                esi = esi.wrapping_shl((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                esi = (esi & 0xff0000);
                esi = (esi | 0x20002050);
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x1c), eax);
                m.write32(esp.wrapping_add(0x18), 0);
                if (fas <= fbs) { pc = 0x310f6b; continue; }
                pc = 0x310dce; continue;
            }
            0x310dce => {
                edi = eax.wrapping_add(0x4b);
                pc = 0x310dd1; continue;
            }
            0x310dd1 => {
                eax = ((m.read8(edi)) as u8 as u32);
                edx = m.read32(ecx.wrapping_add(0x2f18));
                xmm1 = Xmm::scalar(m.read32(edx.wrapping_add(0x8)));
                xmm2.lanes[0] = (((m.read32(ecx.wrapping_add(0x264))) as i32) as f32).to_bits();
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm2.lanes[0]) as f64;
                eax = (((eax & 0xffff00ffu32)) | ((((((if ((((fca == fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x40u32 } else { 0x0u32 }) | (if ((((fca != fca) || (fcb != fcb)))) { 0x4u32 } else { 0x0u32 })) | (if ((((fca < fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x1u32 } else { 0x0u32 })) | 0x2u32)).wrapping_shl((0x8 & 31))));
                fa = (((((((eax) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x44i32)) as u32) & 0xffu32);
                if (((((fa) & (fb))) as u8).count_ones() % 2 == 0) { pc = 0x310f50; continue; }
                pc = 0x310dfc; continue;
            }
            0x310dfc => {
                eax = edx;
                edx = ((m.read8(edi.wrapping_add(((0x2i32.wrapping_neg()) as u32)))) as u8 as u32);
                xmm1.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax))).to_bits();
                edx = sse::cvttss2si(f32::from_bits(xmm1.lanes[0]));
                write16(m, esp.wrapping_add(0x10), (((((edx) as u16)) as u32)) as u16);
                edx = ((m.read8(edi.wrapping_add(((0x1i32.wrapping_neg()) as u32)))) as u8 as u32);
                xmm1.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x4)))).to_bits();
                edx = sse::cvttss2si(f32::from_bits(xmm1.lanes[0]));
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x8)));
                fa = ((((m.read8(edi.wrapping_add(0x2)))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                write16(m, esp.wrapping_add(0x12), (((((edx) as u16)) as u32)) as u16);
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                edx = sse::cvttss2si(f32::from_bits(xmm1.lanes[0]));
                write16(m, esp.wrapping_add(0x14), (((((edx) as u16)) as u32)) as u16);
                if (fa != fb) { pc = 0x310efd; continue; }
                pc = 0x310e45; continue;
            }
            0x310e45 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(edi.wrapping_add(0x1))) as u32)) & 0xff);
                ebx = 0;
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa <= fb) { pc = 0x310f50; continue; }
                pc = 0x310e52; continue;
            }
            0x310e52 => {
                eax = ((m.read8(edi.wrapping_add(((0x3i32.wrapping_neg()) as u32)))) as u8 as u32);
                eax = eax.wrapping_sub(1);
                fa = eax;
                if ((fa == 0)) { pc = 0x310e87; continue; }
                pc = 0x310e59; continue;
            }
            0x310e59 => {
                eax = eax.wrapping_sub(0x2);
                fa = eax;
                if ((fa == 0)) { pc = 0x310e87; continue; }
                pc = 0x310e5e; continue;
            }
            0x310e5e => {
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                eax = 0;
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x310e6f; continue; }
                pc = 0x310e6c; continue;
            }
            0x310e6c => {
                xmm0 = Xmm::default();
                pc = 0x310e6f; continue;
            }
            0x310e6f => {
                ecx = (((((m.read16(esp.wrapping_add(0x12)) as i16)) as i32)) as u32);
                xmm1.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                edx = sse::cvttss2si(f32::from_bits(xmm1.lanes[0]));
                write16(m, esp.wrapping_add(0x12), (((((edx) as u16)) as u32)) as u16);
                pc = 0x310eb1; continue;
            }
            0x310e87 => {
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                eax = 0x1;
                if (fa != fb) { pc = 0x310e9b; continue; }
                pc = 0x310e98; continue;
            }
            0x310e98 => {
                xmm0 = Xmm::default();
                pc = 0x310e9b; continue;
            }
            0x310e9b => {
                ecx = (((((m.read16(esp.wrapping_add(0x10)) as i16)) as i32)) as u32);
                xmm1.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                edx = sse::cvttss2si(f32::from_bits(xmm1.lanes[0]));
                write16(m, esp.wrapping_add(0x10), (((((edx) as u16)) as u32)) as u16);
                pc = 0x310eb1; continue;
            }
            0x310eb1 => {
                ebp = m.read32(esp.wrapping_add(0x10));
                ecx = (ecx & 0xffff_0000) | ((((m.read16(esp.wrapping_add(0x14))) as u32)) & 0xffff);
                push!(eax);
                esp = esp.wrapping_sub(0x8);
                eax = esp;
                m.write32(eax, ebp);
                write16(m, eax.wrapping_add(0x4), (((((ecx) as u16)) as u32)) as u16);
                call!(0x3344e0);
                pc = 0x310ecb; continue;
            }
            0x310ecb => {
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x310eee; continue; }
                pc = 0x310ed2; continue;
            }
            0x310ed2 => {
                eax = ((m.read8(edi.wrapping_add(((0x3i32.wrapping_neg()) as u32)))) as u8 as u32);
                ecx = (ecx & 0xffff_0000) | ((((m.read16(esp.wrapping_add(0x14))) as u32)) & 0xffff);
                esp = esp.wrapping_sub(0x8);
                edx = esp;
                m.write32(edx, ebp);
                write16(m, edx.wrapping_add(0x4), (((((ecx) as u16)) as u32)) as u16);
                call!(0x30ba90);
                pc = 0x310eeb; continue;
            }
            0x310eeb => {
                esp = esp.wrapping_add(0x8);
                pc = 0x310eee; continue;
            }
            0x310eee => {
                edx = ((m.read8(edi.wrapping_add(0x1))) as u8 as u32);
                ebx = ebx.wrapping_add(1);
                fa = ebx;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x310e52; continue; }
                pc = 0x310efb; continue;
            }
            0x310efb => {
                pc = 0x310f4a; continue;
            }
            0x310efd => {
                eax = ((m.read8(edi.wrapping_add(((0x3i32.wrapping_neg()) as u32)))) as u8 as u32);
                eax = eax.wrapping_sub(1);
                fa = eax;
                if ((fa == 0)) { pc = 0x310f0d; continue; }
                pc = 0x310f04; continue;
            }
            0x310f04 => {
                eax = eax.wrapping_sub(0x2);
                fa = eax;
                if ((fa == 0)) { pc = 0x310f0d; continue; }
                pc = 0x310f09; continue;
            }
            0x310f09 => {
                eax = 0;
                pc = 0x310f12; continue;
            }
            0x310f0d => {
                eax = 0x1;
                pc = 0x310f12; continue;
            }
            0x310f12 => {
                ebx = m.read32(esp.wrapping_add(0x10));
                push!(eax);
                esp = esp.wrapping_sub(0x8);
                eax = esp;
                m.write32(eax, ebx);
                write16(m, eax.wrapping_add(0x4), (((((edx) as u16)) as u32)) as u16);
                call!(0x3344e0);
                pc = 0x310f27; continue;
            }
            0x310f27 => {
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x310f4a; continue; }
                pc = 0x310f2e; continue;
            }
            0x310f2e => {
                eax = ((m.read8(edi.wrapping_add(((0x3i32.wrapping_neg()) as u32)))) as u8 as u32);
                edx = (edx & 0xffff_0000) | ((((m.read16(esp.wrapping_add(0x14))) as u32)) & 0xffff);
                esp = esp.wrapping_sub(0x8);
                ecx = esp;
                m.write32(ecx, ebx);
                write16(m, ecx.wrapping_add(0x4), (((((edx) as u16)) as u32)) as u16);
                call!(0x30ba90);
                pc = 0x310f47; continue;
            }
            0x310f47 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x310f4a; continue;
            }
            0x310f4a => {
                ecx = m.read32(0x7844a8);
                pc = 0x310f50; continue;
            }
            0x310f50 => {
                eax = m.read32(esp.wrapping_add(0x18));
                edx = m.read32(esp.wrapping_add(0x1c));
                ebx = m.read32(edx.wrapping_add(0x44));
                eax = eax.wrapping_add(1);
                edi = edi.wrapping_add(0x6);
                fa = eax;
                fb = ebx;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x18), eax);
                if (fas < fbs) { pc = 0x310dd1; continue; }
                pc = 0x310f6b; continue;
            }
            0x310f6b => {
                edx = m.read32(ecx.wrapping_add(0x254));
                fa = edx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x310fb4; continue; }
                pc = 0x310f76; continue;
            }
            0x310f76 => {
                fa = edx;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3110da; continue; }
                pc = 0x310f7f; continue;
            }
            0x310f7f => {
                eax = m.read32(ecx.wrapping_add(0x1185c));
                fa = eax;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x310fb4; continue; }
                pc = 0x310f8a; continue;
            }
            0x310f8a => {
                fa = eax;
                fb = ((((0x5i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x310fb4; continue; }
                pc = 0x310f8f; continue;
            }
            0x310f8f => {
                fa = edx;
                fb = edx;
                if (fa != fb) { pc = 0x3110da; continue; }
                pc = 0x310f97; continue;
            }
            0x310f97 => {
                eax = m.read32(ecx.wrapping_add(0x2f34));
                push!(eax);
                call!(0x2f7da0);
                pc = 0x310fa3; continue;
            }
            0x310fa3 => {
                ecx = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3110da; continue; }
                pc = 0x310fb4; continue;
            }
            0x310fb4 => {
                eax = m.read32(ecx.wrapping_add(0x2f70));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3110da; continue; }
                pc = 0x310fc2; continue;
            }
            0x310fc2 => {
                edx = m.read32(ecx.wrapping_add(0x2f74));
                fa = edx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x3110da; continue; }
                pc = 0x310fd1; continue;
            }
            0x310fd1 => {
                esi = m.read32(eax.wrapping_add(0xc));
                esi = m.read32(eax.wrapping_add(esi.wrapping_mul(0x4)).wrapping_add(0x2c));
                edx = edx.wrapping_add(edx.wrapping_mul(0x2)).wrapping_add(0x24);
                ebx = ((m.read8(esi.wrapping_add(edx.wrapping_mul(0x2)).wrapping_add(0x1))) as u8 as u32);
                edi = esi.wrapping_add(edx.wrapping_mul(0x2));
                esi = ((m.read8(0x7ba17c)) as u8 as u32);
                esi = esi.wrapping_add(0x20);
                xmm1.lanes[0] = (((ebx) as i32) as f32).to_bits();
                ebx = ((m.read8(edi.wrapping_add(0x2))) as u8 as u32);
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax))).to_bits();
                esi = esi.wrapping_shl((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                esi = (esi & 0xff0000);
                xmm0.lanes[0] = (((ebx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x4)))).to_bits();
                eax = (eax & 0xffff_ff00) | ((((m.read8(edi)) as u32)) & 0xff);
                esi = (esi | 0x80002050u32);
                edx = 0;
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x7i32)) as u32) & 0xffu32);
                if (fa > fb) { pc = 0x31108e; continue; }
                pc = 0x31101a; continue;
            }
            0x31101a => {
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                eax = m.read32(0x7ba9e0);
                if ((fa & fb) == 0) { pc = 0x31105c; continue; }
                pc = 0x311023; continue;
            }
            0x311023 => {
                xmm1.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                eax = ((eax as i32).wrapping_mul((0x1e0) as i32)) as u32;
                xmm2 = Xmm::scalar(m.read32(eax.wrapping_add(0x7ba280)));
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783dd4))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                edx = sse::cvttss2si(f32::from_bits(xmm2.lanes[0]));
                xmm1.lanes[0] = (((edx) as i32) as f32).to_bits();
                edx = 0x1;
                pc = 0x31108e; continue;
            }
            0x31105c => {
                xmm0.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                eax = ((eax as i32).wrapping_mul((0x1e0) as i32)) as u32;
                xmm2 = Xmm::scalar(m.read32(eax.wrapping_add(0x7ba284)));
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783f20))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                eax = sse::cvttss2si(f32::from_bits(xmm2.lanes[0]));
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                pc = 0x31108e; continue;
            }
            0x31108e => {
                ebx = (ebx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(0x264))) as u32)) & 0xffff);
                eax = sse::cvttss2si(f32::from_bits(xmm1.lanes[0]));
                write16(m, esp.wrapping_add(0x1c), (((((eax) as u16)) as u32)) as u16);
                push!(edx);
                eax = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                esp = esp.wrapping_sub(0x8);
                write16(m, esp.wrapping_add(0x2a), (((((eax) as u16)) as u32)) as u16);
                ebp = m.read32(esp.wrapping_add(0x28));
                ecx = esp;
                m.write32(ecx, ebp);
                write16(m, ecx.wrapping_add(0x4), (((((ebx) as u16)) as u32)) as u16);
                call!(0x3344e0);
                pc = 0x3110bc; continue;
            }
            0x3110bc => {
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3111bd; continue; }
                pc = 0x3110c7; continue;
            }
            0x3110c7 => {
                eax = ((m.read8(edi)) as u8 as u32);
                esp = esp.wrapping_sub(0x8);
                edx = esp;
                m.write32(edx, ebp);
                write16(m, edx.wrapping_add(0x4), (((((ebx) as u16)) as u32)) as u16);
                pc = 0x3111b5; continue;
            }
            0x3110da => {
                eax = m.read32(ecx.wrapping_add(0x254));
                fa = eax;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x311102; continue; }
                pc = 0x3110e5; continue;
            }
            0x3110e5 => {
                fa = eax;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3111c3; continue; }
                pc = 0x3110ee; continue;
            }
            0x3110ee => {
                eax = m.read32(ecx.wrapping_add(0x1185c));
                fa = eax;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x311102; continue; }
                pc = 0x3110f9; continue;
            }
            0x3110f9 => {
                fa = eax;
                fb = ((((0x5i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3111c3; continue; }
                pc = 0x311102; continue;
            }
            0x311102 => {
                edi = m.read32(ecx.wrapping_add(0x2f78));
                fa = edi;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x3111c3; continue; }
                pc = 0x311111; continue;
            }
            0x311111 => {
                esi = ((m.read8(0x7ba17c)) as u8 as u32);
                eax = edi;
                eax = eax.wrapping_add(eax.wrapping_mul(0x2));
                edx = (edx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x3070))) as u32)) & 0xffff);
                ebx = (ebx & 0xffff_ff00) | ((((m.read8(ecx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x3076))) as u32)) & 0xff);
                eax = ecx.wrapping_add(eax.wrapping_mul(0x4));
                esi = esi.wrapping_add(0x20);
                esi = esi.wrapping_shl((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                write16(m, esp.wrapping_add(0x1c), (((((edx) as u16)) as u32)) as u16);
                edx = (edx & 0xffff_0000) | ((((m.read16(eax.wrapping_add(0x3072))) as u32)) & 0xffff);
                esi = (esi & 0xff0000);
                esi = (esi | 0x80002050u32);
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((0xbi32)) as u32) & 0xffu32);
                write16(m, esp.wrapping_add(0x1e), (((((edx) as u16)) as u32)) as u16);
                edx = (edx & 0xffff_0000) | ((((m.read16(eax.wrapping_add(0x3074))) as u32)) & 0xffff);
                eax = edi.wrapping_add(edi.wrapping_mul(0x2));
                if (fa < fb) { pc = 0x311190; continue; }
                pc = 0x311161; continue;
            }
            0x311161 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(ecx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x3076))) as u32)) & 0xff);
                write16(m, esp.wrapping_add(0x1e), (((((m.read16(esp.wrapping_add(0x1e))) as i32).wrapping_add(0x1i32)) as u32)) as u16);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0xbi32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x311177; continue; }
                pc = 0x311172; continue;
            }
            0x311172 => {
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0xdi32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x31117c; continue; }
                pc = 0x311177; continue;
            }
            0x311177 => {
                write16(m, esp.wrapping_add(0x1c), (((((m.read16(esp.wrapping_add(0x1c))) as i32).wrapping_add(0x1i32)) as u32)) as u16);
                pc = 0x31117c; continue;
            }
            0x31117c => {
                edi = m.read32(esp.wrapping_add(0x1c));
                eax = ((((ecx) as u8)) as u8 as u32);
                esp = esp.wrapping_sub(0x8);
                ecx = esp;
                m.write32(ecx, edi);
                write16(m, ecx.wrapping_add(0x4), (((((edx) as u16)) as u32)) as u16);
                pc = 0x3111b5; continue;
            }
            0x311190 => {
                ebx = (ebx & 0xffff_ff00) | ((((m.read8(ecx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x3076))) as u32)) & 0xff);
                eax = m.read32(esp.wrapping_add(0x1c));
                esp = esp.wrapping_sub(0x8);
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                ecx = esp;
                m.write32(ecx, eax);
                write16(m, ecx.wrapping_add(0x4), (((((edx) as u16)) as u32)) as u16);
                eax = 0x1;
                if ((fa & fb) != 0) { pc = 0x3111b5; continue; }
                pc = 0x3111b0; continue;
            }
            0x3111b0 => {
                eax = 0x2;
                pc = 0x3111b5; continue;
            }
            0x3111b5 => {
                call!(0x30ba90);
                pc = 0x3111ba; continue;
            }
            0x3111ba => {
                esp = esp.wrapping_add(0x8);
                pc = 0x3111bd; continue;
            }
            0x3111bd => {
                ecx = m.read32(0x7844a8);
                pc = 0x3111c3; continue;
            }
            0x3111c3 => {
                fa = ((((m.read8(ecx.wrapping_add(0x25c)))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x3111d8; continue; }
                pc = 0x3111cc; continue;
            }
            0x3111cc => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x14);
                tail_to!(0x30b560); break 'run;  // direct tail jump
            }
            0x3111d8 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x14);
                tail_to!(0x3108d0); break 'run;  // direct tail jump
            }
            0x3111e4 => {
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
/// 0x00312270: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f312270<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x312270;
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
        { crate::pose::cover(0x312270, previous, pc); previous = pc; }
        match pc {
            0x312270 => {
                push!(esi);
                call!(0x74830);
                pc = 0x312276; continue;
            }
            0x312276 => {
                push!(0x312170);
                push!(0);
                push!(0x42340000);
                push!(0);
                push!(0xa800);
                esi = eax;
                call!(0x75b70);
                pc = 0x312290; continue;
            }
            0x312290 => {
                push!(0xadb);
                call!(0x3d7b0);
                pc = 0x31229a; continue;
            }
            0x31229a => {
                ecx = m.read32(esp.wrapping_add(0x20));
                push!(0x1);
                push!(0x1);
                push!(0x3f000000);
                push!(0x1);
                m.write32(ecx.wrapping_add(0x204), eax);
                m.write32(ecx.wrapping_add(0x218), 0x5);
                call!(0x71b10);
                pc = 0x3122be; continue;
            }
            0x3122be => {
                push!(0x20000);
                call!(0x71c50);
                pc = 0x3122c8; continue;
            }
            0x3122c8 => {
                esp = esp.wrapping_add(0x2c);
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0xb01);
                call!(0x3d7b0);
                pc = 0x3122dd; continue;
            }
            0x3122dd => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79460);
                pc = 0x3122e6; continue;
            }
            0x3122e6 => {
                esp = esp.wrapping_add(0x14);
                push!(0);
                push!(0);
                push!(0);
                push!(0x904);
                push!(0xffffffffu32);
                push!(0x1a13);
                call!(0x3d7b0);
                pc = 0x312300; continue;
            }
            0x312300 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x312309; continue;
            }
            0x312309 => {
                esp = esp.wrapping_add(0x18);
                push!(0);
                push!(0);
                push!(0);
                push!(0x904);
                push!(0xffffffffu32);
                push!(0xae0);
                call!(0x3d7b0);
                pc = 0x312323; continue;
            }
            0x312323 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x31232c; continue;
            }
            0x31232c => {
                push!(0);
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0);
                call!(0x79d60);
                pc = 0x31233d; continue;
            }
            0x31233d => {
                push!(0);
                push!(0);
                call!(0x7d4d0);
                pc = 0x312346; continue;
            }
            0x312346 => {
                push!(0xaff);
                call!(0x3d7b0);
                pc = 0x312350; continue;
            }
            0x312350 => {
                push!(eax);
                call!(0x717d0);
                pc = 0x312356; continue;
            }
            0x312356 => {
                esp = esp.wrapping_add(0x40);
                push!(0x3a);
                call!(0x716d0);
                pc = 0x312360; continue;
            }
            0x312360 => {
                push!(0xb00);
                call!(0x3d7b0);
                pc = 0x31236a; continue;
            }
            0x31236a => {
                push!(eax);
                call!(0x717d0);
                pc = 0x312370; continue;
            }
            0x312370 => {
                push!(0x3b);
                call!(0x716d0);
                pc = 0x312377; continue;
            }
            0x312377 => {
                esp = esp.wrapping_add(0x10);
                call!(0x70e30);
                pc = 0x31237f; continue;
            }
            0x31237f => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x312394; continue; }
                pc = 0x312383; continue;
            }
            0x312383 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x200));
                m.write32(0x52d148, ecx);
                pc = 0x312394; continue;
            }
            0x312394 => {
                push!(0x52d148);
                call!(0x78560);
                pc = 0x31239e; continue;
            }
            0x31239e => {
                esp = esp.wrapping_add(0x4);
                call!(0x783d0);
                pc = 0x3123a6; continue;
            }
            0x3123a6 => {
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0xab5);
                call!(0x3d7b0);
                pc = 0x3123b8; continue;
            }
            0x3123b8 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x3123c1; continue;
            }
            0x3123c1 => {
                push!(0);
                push!(0);
                push!(0);
                push!(0);
                push!(0);
                call!(0x7d680);
                pc = 0x3123d0; continue;
            }
            0x3123d0 => {
                push!(0x3c);
                call!(0x716d0);
                pc = 0x3123d7; continue;
            }
            0x3123d7 => {
                call!(0x71630);
                pc = 0x3123dc; continue;
            }
            0x3123dc => {
                edx = m.read32(0x7844a8);
                push!(eax);
                edx = edx.wrapping_add(0x22c);
                push!(edx);
                call!(0x785c0);
                pc = 0x3123ef; continue;
            }
            0x3123ef => {
                esp = esp.wrapping_add(0x34);
                call!(0x783d0);
                pc = 0x3123f7; continue;
            }
            0x3123f7 => {
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0xab6);
                call!(0x3d7b0);
                pc = 0x312409; continue;
            }
            0x312409 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x312412; continue;
            }
            0x312412 => {
                esp = esp.wrapping_add(0x14);
                push!(0);
                push!(0);
                push!(0);
                push!(0x8800);
                push!(0xffffffffu32);
                push!(0xabb);
                call!(0x3d7b0);
                pc = 0x31242c; continue;
            }
            0x31242c => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x312435; continue;
            }
            0x312435 => {
                push!(0);
                push!(0);
                call!(0x7d4f0);
                pc = 0x31243e; continue;
            }
            0x31243e => {
                eax = m.read32(0x7844a8);
                eax = eax.wrapping_add(0x230);
                push!(eax);
                call!(0x78560);
                pc = 0x31244e; continue;
            }
            0x31244e => {
                esp = esp.wrapping_add(0x24);
                push!(0);
                push!(0);
                push!(0);
                push!(0x8800);
                push!(0xffffffffu32);
                push!(0xab7);
                call!(0x3d7b0);
                pc = 0x312468; continue;
            }
            0x312468 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x312471; continue;
            }
            0x312471 => {
                push!(0);
                push!(0);
                call!(0x7d4f0);
                pc = 0x31247a; continue;
            }
            0x31247a => {
                ecx = m.read32(0x7844a8);
                ecx = ecx.wrapping_add(0x234);
                push!(ecx);
                call!(0x78560);
                pc = 0x31248c; continue;
            }
            0x31248c => {
                esp = esp.wrapping_add(0x24);
                call!(0x783d0);
                pc = 0x312494; continue;
            }
            0x312494 => {
                call!(0x35e960);
                pc = 0x312499; continue;
            }
            0x312499 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x312518; continue; }
                pc = 0x31249d; continue;
            }
            0x31249d => {
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0x1aa2);
                call!(0x3d7b0);
                pc = 0x3124af; continue;
            }
            0x3124af => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x3124b8; continue;
            }
            0x3124b8 => {
                esp = esp.wrapping_add(0x14);
                push!(0);
                push!(0);
                push!(0);
                push!(0x8800);
                push!(0xffffffffu32);
                push!(0x1a44);
                call!(0x3d7b0);
                pc = 0x3124d2; continue;
            }
            0x3124d2 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x3124db; continue;
            }
            0x3124db => {
                push!(0);
                push!(0);
                call!(0x7d4f0);
                pc = 0x3124e4; continue;
            }
            0x3124e4 => {
                esp = esp.wrapping_add(0x20);
                push!(0x312240);
                call!(0x71630);
                pc = 0x3124f1; continue;
            }
            0x3124f1 => {
                push!(eax);
                call!(0x71cf0);
                pc = 0x3124f7; continue;
            }
            0x3124f7 => {
                push!(esi);
                call!(0x38d00);
                pc = 0x3124fd; continue;
            }
            0x3124fd => {
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x312513; continue; }
                pc = 0x312504; continue;
            }
            0x312504 => {
                call!(0x71630);
                pc = 0x312509; continue;
            }
            0x312509 => {
                m.write32(eax.wrapping_add(0xc8), 0x1);
                pc = 0x312513; continue;
            }
            0x312513 => {
                call!(0x783d0);
                pc = 0x312518; continue;
            }
            0x312518 => {
                call!(0x22b90);
                pc = 0x31251d; continue;
            }
            0x31251d => {
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
                pc = 0x312566; continue;
            }
            0x312566 => {
                esp = esp.wrapping_add(0x8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x003133E0: compile-only, below Tier 0; fixture debt, unavailable through run().
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3133e0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3133e0;
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
        { crate::pose::cover(0x3133e0, previous, pc); previous = pc; }
        match pc {
            0x3133e0 => {
                ecx = m.read32(0x7844a8);
                push!(ebx);
                push!(ebp);
                ebp = m.read32(ecx.wrapping_add(0x21c));
                eax = 0;
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = (eax & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                push!(esi);
                push!(edi);
                push!(0);
                ebp = eax;
                call!(0x3356f0);
                pc = 0x313400; continue;
            }
            0x313400 => {
                push!(0x36b);
                call!(0x3d7b0);
                pc = 0x31340a; continue;
            }
            0x31340a => {
                esp = esp.wrapping_add(0x8);
                push!(eax);
                push!(0x424);
                call!(0x3d7b0);
                pc = 0x313418; continue;
            }
            0x313418 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x478904);
                push!(0x762d78);
                call!(0x3c85d4);
                pc = 0x31342b; continue;
            }
            0x31342b => {
                push!(0x313310);
                push!(0);
                push!(0x425c0000);
                push!(0);
                push!(0x8800);
                call!(0x75b70);
                pc = 0x313443; continue;
            }
            0x313443 => {
                push!(0xad5);
                call!(0x3d7b0);
                pc = 0x31344d; continue;
            }
            0x31344d => {
                ebx = m.read32(esp.wrapping_add(0x3c));
                push!(0x1);
                push!(0x1);
                push!(0);
                push!(0x1);
                m.write32(ebx.wrapping_add(0x204), eax);
                m.write32(ebx.wrapping_add(0x218), 0x5);
                call!(0x71b10);
                pc = 0x31346e; continue;
            }
            0x31346e => {
                push!(0x20000);
                call!(0x71c50);
                pc = 0x313478; continue;
            }
            0x313478 => {
                esp = esp.wrapping_add(0x3c);
                push!(0);
                push!(0);
                push!(0);
                call!(0x22b90);
                pc = 0x313486; continue;
            }
            0x313486 => {
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
                pc = 0x3134d4; continue;
            }
            0x3134d4 => {
                esp = esp.wrapping_add(0x10);
                push!(0);
                push!(0);
                push!(0x800);
                push!(0);
                push!(0xab9);
                call!(0x3d7b0);
                pc = 0x3134ec; continue;
            }
            0x3134ec => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x3134f5; continue;
            }
            0x3134f5 => {
                push!(0);
                push!(0);
                call!(0x7d4a0);
                pc = 0x3134fe; continue;
            }
            0x3134fe => {
                push!(0x2c);
                call!(0x716d0);
                pc = 0x313505; continue;
            }
            0x313505 => {
                esp = esp.wrapping_add(0x20);
                edi = 0;
                esi = 0x52cf38;
                pc = 0x313510; continue;
            }
            0x313510 => {
                fa = esi;
                fb = ((((0x52cf90i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x313538; continue; }
                pc = 0x313518; continue;
            }
            0x313518 => {
                fa = esi;
                fb = ((((0x52cfe8i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x313538; continue; }
                pc = 0x313520; continue;
            }
            0x313520 => {
                fa = esi;
                fb = ((((0x52d040i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x313538; continue; }
                pc = 0x313528; continue;
            }
            0x313528 => {
                fa = esi;
                fb = ((((0x52d098i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x313538; continue; }
                pc = 0x313530; continue;
            }
            0x313530 => {
                fa = esi;
                fb = ((((0x52d0f0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31354d; continue; }
                pc = 0x313538; continue;
            }
            0x313538 => {
                edx = m.read32(esi);
                push!(edi);
                push!(edx);
                call!(0x3d7b0);
                pc = 0x313541; continue;
            }
            0x313541 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x31354a; continue;
            }
            0x31354a => {
                esp = esp.wrapping_add(0x8);
                pc = 0x31354d; continue;
            }
            0x31354d => {
                esi = esi.wrapping_add(0x58);
                edi = edi.wrapping_add(1);
                fa = esi;
                fb = ((((0x52d148i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x313510; continue; }
                pc = 0x313559; continue;
            }
            0x313559 => {
                eax = m.read32(0x7844a8);
                eax = eax.wrapping_add(0x204);
                push!(eax);
                call!(0x78590);
                pc = 0x313569; continue;
            }
            0x313569 => {
                call!(0x783d0);
                pc = 0x31356e; continue;
            }
            0x31356e => {
                push!(0xaba);
                push!(0x31a540);
                push!(0x1);
                push!(ebx);
                call!(0x31b560);
                pc = 0x313580; continue;
            }
            0x313580 => {
                esp = esp.wrapping_add(0x14);
                push!(0);
                push!(0);
                push!(0x8000800);
                push!(0x1);
                push!(0x3b7);
                call!(0x3d7b0);
                pc = 0x313598; continue;
            }
            0x313598 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x3135a1; continue;
            }
            0x3135a1 => {
                push!(0);
                push!(0);
                call!(0x7d4a0);
                pc = 0x3135aa; continue;
            }
            0x3135aa => {
                push!(0x2f);
                call!(0x716d0);
                pc = 0x3135b1; continue;
            }
            0x3135b1 => {
                esp = esp.wrapping_add(0x20);
                edi = 0;
                esi = 0x518f6c;
                pc = 0x3135c0; continue;
            }
            0x3135c0 => {
                ecx = (((((m.read16(esi) as i16)) as i32)) as u32);
                edi = edi.wrapping_add(1);
                push!(edi);
                push!(ecx);
                call!(0x3d7b0);
                pc = 0x3135cb; continue;
            }
            0x3135cb => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x3135d4; continue;
            }
            0x3135d4 => {
                esi = esi.wrapping_add(0x44);
                esp = esp.wrapping_add(0x8);
                fa = esi;
                fb = ((((0x5191d0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x3135c0; continue; }
                pc = 0x3135e2; continue;
            }
            0x3135e2 => {
                push!(0xf);
                push!(0x762d78);
                call!(0x71800);
                pc = 0x3135ee; continue;
            }
            0x3135ee => {
                push!(0x784028);
                call!(0x78590);
                pc = 0x3135f8; continue;
            }
            0x3135f8 => {
                esp = esp.wrapping_add(0xc);
                push!(0);
                push!(0x3128f0);
                push!(0);
                push!(0x8800);
                push!(0x1);
                push!(0xb08);
                call!(0x3d7b0);
                pc = 0x313615; continue;
            }
            0x313615 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x31361e; continue;
            }
            0x31361e => {
                esp = esp.wrapping_add(0x18);
                call!(0x783d0);
                pc = 0x313626; continue;
            }
            0x313626 => {
                push!(0);
                push!(0);
                push!(0x8000800);
                push!(0x2);
                push!(0x3b8);
                call!(0x3d7b0);
                pc = 0x31363b; continue;
            }
            0x31363b => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x313644; continue;
            }
            0x313644 => {
                push!(0);
                push!(0);
                call!(0x7d4a0);
                pc = 0x31364d; continue;
            }
            0x31364d => {
                push!(0x30);
                call!(0x716d0);
                pc = 0x313654; continue;
            }
            0x313654 => {
                esp = esp.wrapping_add(0x20);
                esi = 0;
                pc = 0x313660; continue;
            }
            0x313660 => {
                eax = (((((m.read16(esi.wrapping_mul(0x2).wrapping_add(0x53ebcc)) as i16)) as i32)) as u32);
                edx = esi.wrapping_add(0x2);
                push!(edx);
                push!(eax);
                call!(0x3d7b0);
                pc = 0x313672; continue;
            }
            0x313672 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x31367b; continue;
            }
            0x31367b => {
                esp = esp.wrapping_add(0x8);
                esi = esi.wrapping_add(1);
                fa = esi;
                fb = ((((0xai32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x313660; continue; }
                pc = 0x313684; continue;
            }
            0x313684 => {
                push!(0x11);
                push!(0x762d78);
                call!(0x71800);
                pc = 0x313690; continue;
            }
            0x313690 => {
                push!(0x784024);
                call!(0x78590);
                pc = 0x31369a; continue;
            }
            0x31369a => {
                esp = esp.wrapping_add(0xc);
                push!(0);
                push!(0x3128f0);
                push!(0);
                push!(0x8800);
                push!(0);
                push!(0xb09);
                call!(0x3d7b0);
                pc = 0x3136b7; continue;
            }
            0x3136b7 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x3136c0; continue;
            }
            0x3136c0 => {
                esp = esp.wrapping_add(0x18);
                call!(0x783d0);
                pc = 0x3136c8; continue;
            }
            0x3136c8 => {
                push!(0);
                push!(0);
                push!(0x800);
                push!(0x3);
                push!(0xabb);
                call!(0x3d7b0);
                pc = 0x3136dd; continue;
            }
            0x3136dd => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x3136e6; continue;
            }
            0x3136e6 => {
                push!(0);
                push!(0);
                call!(0x7d4a0);
                pc = 0x3136ef; continue;
            }
            0x3136ef => {
                push!(0x32);
                call!(0x716d0);
                pc = 0x3136f6; continue;
            }
            0x3136f6 => {
                esp = esp.wrapping_add(0x20);
                push!(0xffffffffu32);
                push!(0x426);
                call!(0x3d7b0);
                pc = 0x313705; continue;
            }
            0x313705 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x31370e; continue;
            }
            0x31370e => {
                esp = esp.wrapping_add(0x8);
                push!(0);
                push!(0x1917);
                call!(0x3d7b0);
                pc = 0x31371d; continue;
            }
            0x31371d => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x313726; continue;
            }
            0x313726 => {
                esp = esp.wrapping_add(0x8);
                esi = 0x1;
                edi = edi;
                pc = 0x313730; continue;
            }
            0x313730 => {
                push!(esi);
                call!(0x35de60);
                pc = 0x313736; continue;
            }
            0x313736 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x313750; continue; }
                pc = 0x31373d; continue;
            }
            0x31373d => {
                push!(esi);
                push!(esi);
                call!(0x2be6f0);
                pc = 0x313744; continue;
            }
            0x313744 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x31374d; continue;
            }
            0x31374d => {
                esp = esp.wrapping_add(0x8);
                pc = 0x313750; continue;
            }
            0x313750 => {
                esi = esi.wrapping_add(1);
                fa = esi;
                fb = ((((0x3ai32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x313730; continue; }
                pc = 0x313756; continue;
            }
            0x313756 => {
                ecx = m.read32(0x7844a8);
                ecx = ecx.wrapping_add(0x210);
                push!(ecx);
                call!(0x78590);
                pc = 0x313768; continue;
            }
            0x313768 => {
                esp = esp.wrapping_add(0x4);
                call!(0x783d0);
                pc = 0x313770; continue;
            }
            0x313770 => {
                push!(0);
                push!(0);
                push!(0);
                push!(0x4);
                push!(0xabc);
                call!(0x3d7b0);
                pc = 0x313782; continue;
            }
            0x313782 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x31378b; continue;
            }
            0x31378b => {
                esp = esp.wrapping_add(0x14);
                push!(0);
                push!(0);
                push!(0);
                push!(0x8800);
                push!(0xffffffffu32);
                push!(0xabd);
                call!(0x3d7b0);
                pc = 0x3137a5; continue;
            }
            0x3137a5 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x3137ae; continue;
            }
            0x3137ae => {
                push!(0);
                push!(0);
                call!(0x7d4f0);
                pc = 0x3137b7; continue;
            }
            0x3137b7 => {
                push!(0x33);
                call!(0x716d0);
                pc = 0x3137be; continue;
            }
            0x3137be => {
                edx = m.read32(0x7844a8);
                edx = edx.wrapping_add(0x214);
                push!(edx);
                call!(0x78560);
                pc = 0x3137d0; continue;
            }
            0x3137d0 => {
                esp = esp.wrapping_add(0x28);
                call!(0x783d0);
                pc = 0x3137d8; continue;
            }
            0x3137d8 => {
                push!(0);
                push!(0);
                eax = ebp;
                eax = (eax | 0x800);
                push!(eax);
                push!(0xffffffffu32);
                push!(0xabe);
                call!(0x3d7b0);
                pc = 0x3137f0; continue;
            }
            0x3137f0 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x3137f9; continue;
            }
            0x3137f9 => {
                push!(0);
                push!(ebp);
                call!(0x7d4a0);
                pc = 0x313801; continue;
            }
            0x313801 => {
                push!(0x35);
                call!(0x716d0);
                pc = 0x313808; continue;
            }
            0x313808 => {
                esp = esp.wrapping_add(0x20);
                esi = 0x52d154;
                pc = 0x313810; continue;
            }
            0x313810 => {
                ecx = m.read32(esi.wrapping_add(((0x4i32.wrapping_neg()) as u32)));
                edx = m.read32(esi);
                push!(ecx);
                push!(edx);
                call!(0x3d7b0);
                pc = 0x31381c; continue;
            }
            0x31381c => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x313825; continue;
            }
            0x313825 => {
                esi = esi.wrapping_add(0x8);
                esp = esp.wrapping_add(0x8);
                fa = esi;
                fb = ((((0x52d1aci32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x313810; continue; }
                pc = 0x313833; continue;
            }
            0x313833 => {
                eax = m.read32(0x7844a8);
                eax = eax.wrapping_add(0x220);
                push!(eax);
                call!(0x78590);
                pc = 0x313843; continue;
            }
            0x313843 => {
                esp = esp.wrapping_add(0x4);
                push!(0);
                push!(0);
                push!(0);
                push!(0x8800);
                push!(0xffffffffu32);
                push!(0xabf);
                call!(0x3d7b0);
                pc = 0x31385d; continue;
            }
            0x31385d => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x313866; continue;
            }
            0x313866 => {
                push!(0x31);
                call!(0x716d0);
                pc = 0x31386d; continue;
            }
            0x31386d => {
                push!(0);
                push!(0);
                call!(0x7d4f0);
                pc = 0x313876; continue;
            }
            0x313876 => {
                ecx = m.read32(0x7844a8);
                ecx = ecx.wrapping_add(0x21c);
                push!(ecx);
                call!(0x78560);
                pc = 0x313888; continue;
            }
            0x313888 => {
                esp = esp.wrapping_add(0x28);
                call!(0x783d0);
                pc = 0x313890; continue;
            }
            0x313890 => {
                push!(0);
                push!(0);
                edx = ebp;
                edx = (edx | 0x800);
                push!(edx);
                push!(0xffffffffu32);
                push!(0xac0);
                call!(0x3d7b0);
                pc = 0x3138a9; continue;
            }
            0x3138a9 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x3138b2; continue;
            }
            0x3138b2 => {
                push!(0);
                push!(ebp);
                call!(0x7d4a0);
                pc = 0x3138ba; continue;
            }
            0x3138ba => {
                push!(0x36);
                call!(0x716d0);
                pc = 0x3138c1; continue;
            }
            0x3138c1 => {
                esp = esp.wrapping_add(0x20);
                esi = 0x52d1ac;
                pc = 0x3138d0; continue;
            }
            0x3138d0 => {
                eax = ((m.read8(esi.wrapping_add(((0x4i32.wrapping_neg()) as u32)))) as u8 as u32);
                ecx = m.read32(esi);
                push!(eax);
                push!(ecx);
                call!(0x3d7b0);
                pc = 0x3138dd; continue;
            }
            0x3138dd => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x3138e6; continue;
            }
            0x3138e6 => {
                esi = esi.wrapping_add(0x8);
                esp = esp.wrapping_add(0x8);
                fa = esi;
                fb = ((((0x52d1cci32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x3138d0; continue; }
                pc = 0x3138f4; continue;
            }
            0x3138f4 => {
                edx = m.read32(0x7844a8);
                edx = edx.wrapping_add(0x224);
                push!(edx);
                call!(0x78590);
                pc = 0x313906; continue;
            }
            0x313906 => {
                esp = esp.wrapping_add(0x4);
                call!(0x783d0);
                pc = 0x31390e; continue;
            }
            0x31390e => {
                push!(0);
                push!(0);
                push!(0x800);
                push!(0xffffffffu32);
                push!(0xac6);
                call!(0x3d7b0);
                pc = 0x313923; continue;
            }
            0x313923 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x31392c; continue;
            }
            0x31392c => {
                esp = esp.wrapping_add(0x14);
                edi = 0x762f38;
                esi = 0x52d1fc;
                pc = 0x313940; continue;
            }
            0x313940 => {
                eax = m.read32(esi);
                push!(0);
                push!(0);
                eax = eax.wrapping_shl((((((((0x5i32)) as u32) & 0x1fu32))) as u32 & 31));
                push!(0);
                eax = eax.wrapping_add(0x4d1f78);
                eax = (((((m.read16(eax.wrapping_add(0x18)) as i16)) as i32)) as u32);
                push!(0x8800);
                push!(0xffffffffu32);
                push!(eax);
                call!(0x3d7b0);
                pc = 0x313961; continue;
            }
            0x313961 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x31396a; continue;
            }
            0x31396a => {
                push!(0);
                push!(0);
                call!(0x7d4f0);
                pc = 0x313973; continue;
            }
            0x313973 => {
                push!(edi);
                call!(0x78560);
                pc = 0x313979; continue;
            }
            0x313979 => {
                esi = esi.wrapping_add(0x24);
                esp = esp.wrapping_add(0x24);
                edi = edi.wrapping_add(0x4);
                fa = esi;
                fb = ((((0x52d3f4i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x313940; continue; }
                pc = 0x31398a; continue;
            }
            0x31398a => {
                call!(0x783d0);
                pc = 0x31398f; continue;
            }
            0x31398f => {
                push!(0);
                push!(0);
                push!(0x800);
                push!(0x6);
                push!(0xac5);
                call!(0x3d7b0);
                pc = 0x3139a4; continue;
            }
            0x3139a4 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x3139ad; continue;
            }
            0x3139ad => {
                push!(0);
                push!(0);
                push!(0);
                push!(0xdf);
                push!(0x783e00);
                push!(0);
                push!(0);
                call!(0x7d560);
                pc = 0x3139c6; continue;
            }
            0x3139c6 => {
                push!(0x37);
                call!(0x716d0);
                pc = 0x3139cd; continue;
            }
            0x3139cd => {
                esp = esp.wrapping_add(0x34);
                call!(0x783d0);
                pc = 0x3139d5; continue;
            }
            0x3139d5 => {
                push!(0);
                push!(0);
                push!(0x800);
                push!(0x8);
                push!(0x3db);
                call!(0x3d7b0);
                pc = 0x3139ea; continue;
            }
            0x3139ea => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x3139f3; continue;
            }
            0x3139f3 => {
                push!(0);
                push!(0);
                push!(0);
                push!(0xdf);
                push!(0x783ce0);
                push!(0);
                push!(0);
                call!(0x7d560);
                pc = 0x313a0c; continue;
            }
            0x313a0c => {
                push!(0x38);
                call!(0x716d0);
                pc = 0x313a13; continue;
            }
            0x313a13 => {
                call!(0x783d0);
                pc = 0x313a18; continue;
            }
            0x313a18 => {
                call!(0x781a0);
                pc = 0x313a1d; continue;
            }
            0x313a1d => {
                call!(0x75e70);
                pc = 0x313a22; continue;
            }
            0x313a22 => {
                call!(0x22b90);
                pc = 0x313a27; continue;
            }
            0x313a27 => {
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
                esp = esp.wrapping_add(0x30);
                m.write32(esp, xmm1.lanes[0]);
                push!(ecx);
                call!(0x77320);
                pc = 0x313a72; continue;
            }
            0x313a72 => {
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

/// 0x00314420: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f314420<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x314420;
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
        { crate::pose::cover(0x314420, previous, pc); previous = pc; }
        match pc {
            0x314420 => {
                push!(ecx);
                push!(ebx);
                push!(ebp);
                push!(esi);
                push!(edi);
                call!(0x2fdb40);
                pc = 0x31442a; continue;
            }
            0x31442a => {
                push!(0);
                m.write32(esp.wrapping_add(0x14), eax);
                eax = m.read32(0x4c0454);
                ecx = m.read32(eax.wrapping_mul(0x8).wrapping_add(0x52d440));
                push!(0);
                push!(ecx);
                push!(0);
                push!(0xa800);
                call!(0x75b70);
                pc = 0x31444b; continue;
            }
            0x31444b => {
                edx = m.read32(esp.wrapping_add(0x2c));
                push!(0x1);
                push!(0x1);
                push!(0x3fc00000);
                push!(0x1);
                m.write32(edx.wrapping_add(0x218), 0x1);
                call!(0x71b10);
                pc = 0x314469; continue;
            }
            0x314469 => {
                push!(0x20000);
                call!(0x71c50);
                pc = 0x314473; continue;
            }
            0x314473 => {
                esp = esp.wrapping_add(0x28);
                push!(0);
                push!(0);
                push!(0);
                call!(0x22b90);
                pc = 0x314481; continue;
            }
            0x314481 => {
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
                pc = 0x3144cf; continue;
            }
            0x3144cf => {
                esp = esp.wrapping_add(0x10);
                ebx = 0;
                edi = 0x52d1e8;
                pc = 0x3144e0; continue;
            }
            0x3144e0 => {
                esi = m.read32(edi.wrapping_add(((0x10i32.wrapping_neg()) as u32)));
                eax = m.read32(esp.wrapping_add(0x10));
                ecx = m.read32(0x7844a8);
                ecx = m.read32(ecx.wrapping_add(0x200));
                edx = (edx & 0xffff_ff00) | ((((m.read8(edi.wrapping_add(0xc))) as u32)) & 0xff);
                esi = esi.wrapping_shl((((((((0x5i32)) as u32) & 0x1fu32))) as u32 & 31));
                esi = esi.wrapping_add(0x4d1f78);
                ebp = m.read32(esi.wrapping_add(0x8));
                ebp = (ebp & eax);
                fa = ebp;
                fas = (fa as i32);
                eax = 0x1;
                if !(((((((((ecx) as u8))) as u32) & 0x1fu32))) != 0) { fa = if (((fa == 0))) { 0x0u32 } else { (if (((fas < 0i32))) { 0x80000000u32 } else { 0x1u32 }) }; fas = ((fa) as i32); }
                eax = eax.wrapping_shl((((((((((ecx) as u8))) as u32) & 0x1fu32))) as u32 & 31));
                if (((((((((ecx) as u8))) as u32) & 0x1fu32))) != 0) { fa = eax; fas = (fa as i32); }
                fa = ((((((edx) as u8))) as u32) & 0xffu32);
                fb = ((((((eax) as u8))) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x3145d0; continue; }
                pc = 0x314513; continue;
            }
            0x314513 => {
                eax = m.read32(ebx.wrapping_mul(0x4).wrapping_add(0x762f34));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x314551; continue; }
                pc = 0x31451e; continue;
            }
            0x31451e => {
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x314551; continue; }
                pc = 0x314522; continue;
            }
            0x314522 => {
                eax = (((((m.read16(esi.wrapping_add(0x18)) as i16)) as i32)) as u32);
                push!(0);
                push!(0x313fa0);
                push!(0);
                push!(0xc00);
                push!(ebx);
                push!(eax);
                call!(0x3d7b0);
                pc = 0x31453b; continue;
            }
            0x31453b => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x314544; continue;
            }
            0x314544 => {
                ecx = m.read32(edi);
                push!(ecx);
                call!(0x716d0);
                pc = 0x31454c; continue;
            }
            0x31454c => {
                esp = esp.wrapping_add(0x1c);
                pc = 0x3145d0; continue;
            }
            0x314551 => {
                push!(0x2);
                push!(0);
                push!(0);
                call!(0x78320);
                pc = 0x31455c; continue;
            }
            0x31455c => {
                esp = esp.wrapping_add(0xc);
                push!(0);
                push!(0);
                push!(0);
                call!(0x71c60);
                pc = 0x31456a; continue;
            }
            0x31456a => {
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(0x4760cc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                edx = (((((m.read16(esi.wrapping_add(0x18)) as i16)) as i32)) as u32);
                push!(ecx);
                m.write32(esp, (fpu.st(0) as f32).to_bits()); fpu.pop();
                push!(0x4000401);
                push!(ebx);
                push!(edx);
                call!(0x3d7b0);
                pc = 0x314584; continue;
            }
            0x314584 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x792b0);
                pc = 0x31458d; continue;
            }
            0x31458d => {
                eax = m.read32(edi);
                push!(eax);
                call!(0x716d0);
                pc = 0x314595; continue;
            }
            0x314595 => {
                esp = esp.wrapping_add(0x20);
                push!(0x312610);
                push!(0x3126d0);
                push!(0);
                push!(0);
                push!(0x30000);
                push!(ebx);
                push!(0xae2);
                call!(0x3d7b0);
                pc = 0x3145b6; continue;
            }
            0x3145b6 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x792b0);
                pc = 0x3145bf; continue;
            }
            0x3145bf => {
                ecx = m.read32(edi.wrapping_add(0x4));
                push!(ecx);
                call!(0x716d0);
                pc = 0x3145c8; continue;
            }
            0x3145c8 => {
                esp = esp.wrapping_add(0x20);
                call!(0x78380);
                pc = 0x3145d0; continue;
            }
            0x3145d0 => {
                edi = edi.wrapping_add(0x24);
                ebx = ebx.wrapping_add(1);
                fa = edi;
                fb = ((((0x52d404i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x3144e0; continue; }
                pc = 0x3145e0; continue;
            }
            0x3145e0 => {
                call!(0x781a0);
                pc = 0x3145e5; continue;
            }
            0x3145e5 => {
                call!(0x75e70);
                pc = 0x3145ea; continue;
            }
            0x3145ea => {
                call!(0x22b90);
                pc = 0x3145ef; continue;
            }
            0x3145ef => {
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
                pc = 0x314638; continue;
            }
            0x314638 => {
                esp = esp.wrapping_add(0x8);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00314F80: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f314f80<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x314f80;
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
        { crate::pose::cover(0x314f80, previous, pc); previous = pc; }
        match pc {
            0x314f80 => {
                eax = m.read32(esp.wrapping_add(0x4));
                push!(eax);
                call!(0x314d70);
                pc = 0x314f8a; continue;
            }
            0x314f8a => {
                esp = esp.wrapping_add(0x4);
                eax = 0x1;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00314FA0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f314fa0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x314fa0;
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
        { crate::pose::cover(0x314fa0, previous, pc); previous = pc; }
        match pc {
            0x314fa0 => {
                push!(ebx);
                push!(esi);
                push!(edi);
                edi = 0;
                ebx = 0;
                esi = 0;
                call!(0x314b70);
                pc = 0x314fae; continue;
            }
            0x314fae => {
                eax = m.read32(0x7ba9e0);
                eax = ((eax as i32).wrapping_mul((0x1e0) as i32)) as u32;
                ecx = m.read32(eax.wrapping_add(0x7ba26c));
                push!(ecx);
                call!(0x77470);
                pc = 0x314fc5; continue;
            }
            0x314fc5 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                if ((fa & fb) == 0) { pc = 0x314fdb; continue; }
                pc = 0x314fd6; continue;
            }
            0x314fd6 => {
                tail_to!(0x314d70); break 'run;  // direct tail jump
            }
            0x314fdb => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00315D20: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f315d20<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x315d20;
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
        { crate::pose::cover(0x315d20, previous, pc); previous = pc; }
        match pc {
            0x315d20 => {
                eax = m.read32(esp.wrapping_add(0x4));
                eax = ((eax as i32).wrapping_mul((0xc4) as i32)) as u32;
                ecx = m.read32(eax.wrapping_add(0x525010));
                fa = (((((((ecx) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x315d44; continue; }
                pc = 0x315d35; continue;
            }
            0x315d35 => {
                eax = m.read32(eax.wrapping_add(0x52500c));
                m.write32(esp.wrapping_add(0x4), eax);
                tail_to!(0x80f20); break 'run;  // direct tail jump
            }
            0x315d44 => {
                ecx = m.read32(eax.wrapping_add(0x52500c));
                m.write32(esp.wrapping_add(0x4), ecx);
                tail_to!(0x81c10); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00315D60: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f315d60<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x315d60;
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
        { crate::pose::cover(0x315d60, previous, pc); previous = pc; }
        match pc {
            0x315d60 => {
                ecx = m.read32(esp.wrapping_add(0x4));
                push!(esi);
                esi = ecx.wrapping_add(ecx.wrapping_mul(0x2));
                esi = esi.wrapping_shl((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = m.read32(esi.wrapping_add(0x52dc48));
                push!(edi);
                eax = 0;
                edi = 0;
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x315da8; continue; }
                pc = 0x315d7a; continue;
            }
            0x315d7a => {
                pc = 0x315d80; continue;
            }
            0x315d80 => {
                eax = m.read32(esi.wrapping_add(0x52dc44));
                ecx = m.read32(eax.wrapping_add(edi.wrapping_mul(0x4)));
                edx = m.read32(0x784014);
                push!(ecx);
                push!(edx);
                call!(0x32a650);
                pc = 0x315d96; continue;
            }
            0x315d96 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x315da8; continue; }
                pc = 0x315d9d; continue;
            }
            0x315d9d => {
                ecx = m.read32(esi.wrapping_add(0x52dc48));
                edi = edi.wrapping_add(1);
                fa = edi;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x315d80; continue; }
                pc = 0x315da8; continue;
            }
            0x315da8 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00315DB0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f315db0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x315db0;
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
        { crate::pose::cover(0x315db0, previous, pc); previous = pc; }
        match pc {
            0x315db0 => {
                ecx = m.read32(esp.wrapping_add(0x4));
                push!(ebx);
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0x10));
                push!(esi);
                ebx = 0;
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                push!(edi);
                eax = 0x1;
                if (fas <= fbs) { pc = 0x315e02; continue; }
                pc = 0x315dc7; continue;
            }
            0x315dc7 => {
                edi = edx;
                edi = edi.wrapping_add(0x4);
                pc = 0x315dd0; continue;
            }
            0x315dd0 => {
                esi = m.read32(edi.wrapping_add(0x4));
                ecx = 0;
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x315dec; continue; }
                pc = 0x315dd9; continue;
            }
            0x315dd9 => {
                edx = m.read32(edi);
                pc = 0x315de0; continue;
            }
            0x315de0 => {
                fa = ebp;
                fb = m.read32(edx);
                if (fa == fb) { pc = 0x315e00; continue; }
                pc = 0x315de4; continue;
            }
            0x315de4 => {
                ecx = ecx.wrapping_add(1);
                edx = edx.wrapping_add(0x4);
                fa = ecx;
                fb = esi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x315de0; continue; }
                pc = 0x315dec; continue;
            }
            0x315dec => {
                eax = eax.wrapping_add(m.read32(edi.wrapping_add(0x4)));
                ecx = m.read32(esp.wrapping_add(0x14));
                ebx = ebx.wrapping_add(1);
                edi = edi.wrapping_add(0xc);
                fa = ebx;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x315dd0; continue; }
                pc = 0x315dfb; continue;
            }
            0x315dfb => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x315e00 => {
                eax = eax.wrapping_add(ecx);
                pc = 0x315e02; continue;
            }
            0x315e02 => {
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

/// 0x00315E10: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f315e10<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x315e10;
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
        { crate::pose::cover(0x315e10, previous, pc); previous = pc; }
        match pc {
            0x315e10 => {
                ecx = m.read32(0x7844a8);
                ecx = m.read32(ecx.wrapping_add(0x200));
                eax = 0;
                ecx = ecx.wrapping_sub(eax);
                fa = ecx;
                if ((fa == 0)) { pc = 0x315e3a; continue; }
                pc = 0x315e22; continue;
            }
            0x315e22 => {
                ecx = ecx.wrapping_sub(1);
                fa = ecx;
                if ((fa != 0)) { pc = 0x315e4e; continue; }
                pc = 0x315e25; continue;
            }
            0x315e25 => {
                edx = m.read32(esp.wrapping_add(0x4));
                push!(edx);
                push!(0x11);
                edx = 0x52dc40;
                call!(0x315db0);
                pc = 0x315e36; continue;
            }
            0x315e36 => {
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            0x315e3a => {
                eax = m.read32(esp.wrapping_add(0x4));
                push!(eax);
                push!(0x9);
                edx = 0x52dd10;
                call!(0x315db0);
                pc = 0x315e4b; continue;
            }
            0x315e4b => {
                esp = esp.wrapping_add(0x8);
                pc = 0x315e4e; continue;
            }
            0x315e4e => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00316110: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f316110<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x316110;
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
        { crate::pose::cover(0x316110, previous, pc); previous = pc; }
        match pc {
            0x316110 => {
                esp = esp.wrapping_sub(0xc);
                push!(ebx);
                push!(ebp);
                push!(esi);
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x24));
                ebx = m.read32(edi.wrapping_add(0x14));
                esi = ebx;
                esi = ((esi as i32).wrapping_mul((0xc4) as i32)) as u32;
                ebp = m.read32(esi.wrapping_add(0x524f5c));
                push!(edi);
                call!(0x757e0);
                pc = 0x316132; continue;
            }
            0x316132 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x316167; continue; }
                pc = 0x316139; continue;
            }
            0x316139 => {
                eax = esp.wrapping_add(0x10);
                push!(eax);
                push!(ebp);
                call!(0x154bc0);
                pc = 0x316144; continue;
            }
            0x316144 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x20)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475cd4))).to_bits();
                ecx = esp.wrapping_add(0x18);
                push!(ecx);
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                call!(0x154b50);
                pc = 0x316162; continue;
            }
            0x316162 => {
                esp = esp.wrapping_add(0xc);
                ebp = eax;
                pc = 0x316167; continue;
            }
            0x316167 => {
                eax = m.read32(esi.wrapping_add(0x525010));
                fa = (((((((eax) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x316180; continue; }
                pc = 0x316172; continue;
            }
            0x316172 => {
                edx = m.read32(esi.wrapping_add(0x52500c));
                push!(edx);
                call!(0x80f20);
                pc = 0x31617e; continue;
            }
            0x31617e => {
                pc = 0x31618c; continue;
            }
            0x316180 => {
                eax = m.read32(esi.wrapping_add(0x52500c));
                push!(eax);
                call!(0x81c10);
                pc = 0x31618c; continue;
            }
            0x31618c => {
                esp = esp.wrapping_add(0x4);
                push!(ebx);
                esi = eax;
                call!(0x308340);
                pc = 0x316197; continue;
            }
            0x316197 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0xa0a0a20);
                push!(ebp);
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x38));
                push!(esi);
                push!(edi);
                call!(0x757e0);
                pc = 0x3161ad; continue;
            }
            0x3161ad => {
                ebx = m.read32(esp.wrapping_add(0x38));
                ecx = m.read32(ebx.wrapping_add(0xf4));
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(ecx);
                call!(0x315e50);
                pc = 0x3161c1; continue;
            }
            0x3161c1 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(edi.wrapping_add(0x28))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x1c);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x3161f4; continue; }
                pc = 0x3161cb; continue;
            }
            0x3161cb => {
                edx = m.read32(ebx.wrapping_add(0xf4));
                push!(edx);
                push!(0x32323248);
                call!(0x154fe0);
                pc = 0x3161dc; continue;
            }
            0x3161dc => {
                ecx = m.read32(esi.wrapping_add(0x8));
                edx = m.read32(esi.wrapping_add(0x4));
                push!(eax);
                eax = m.read32(esi.wrapping_add(0xc));
                push!(eax);
                eax = m.read32(esi);
                push!(ecx);
                push!(edx);
                push!(eax);
                call!(0x1a2e0);
                pc = 0x3161f1; continue;
            }
            0x3161f1 => {
                esp = esp.wrapping_add(0x1c);
                pc = 0x3161f4; continue;
            }
            0x3161f4 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
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

/// 0x00316B50: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f316b50<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x316b50;
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
        { crate::pose::cover(0x316b50, previous, pc); previous = pc; }
        match pc {
            0x316b50 => {
                eax = m.read32(0x7844a8);
                eax = m.read32(eax.wrapping_add(0x200));
                push!(esi);
                push!(0x2fb090);
                cf = (((((eax) != 0))) as i32);
                eax = (((((eax) as i32).wrapping_neg())) as u32);
                push!(0);
                eax = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                push!(0x42480000);
                eax = (eax & 0x5);
                eax = eax.wrapping_add(0x3);
                push!(0);
                push!(0x840c);
                esi = eax;
                call!(0x75b70);
                pc = 0x316b80; continue;
            }
            0x316b80 => {
                push!(0x1);
                push!(0x1);
                push!(0);
                push!(0);
                call!(0x71b10);
                pc = 0x316b8d; continue;
            }
            0x316b8d => {
                push!(0x28000);
                call!(0x71c50);
                pc = 0x316b97; continue;
            }
            0x316b97 => {
                esp = esp.wrapping_add(0x28);
                push!(0);
                push!(0);
                push!(0);
                call!(0x22b90);
                pc = 0x316ba5; continue;
            }
            0x316ba5 => {
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
                pc = 0x316bf3; continue;
            }
            0x316bf3 => {
                esp = esp.wrapping_add(0x10);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x316cb0; continue; }
                pc = 0x316bfe; continue;
            }
            0x316bfe => {
                push!(ebx);
                push!(ebp);
                push!(edi);
                ebx = 0;
                ebp = esi;
                pc = 0x316c10; continue;
                edi = edi;
                pc = 0x316c10; continue;
            }
            0x316c10 => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x200));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                edi = ebx.wrapping_add(0x52dac0);
                if (fa != fb) { pc = 0x316c2c; continue; }
                pc = 0x316c26; continue;
            }
            0x316c26 => {
                edi = ebx.wrapping_add(0x52db20);
                pc = 0x316c2c; continue;
            }
            0x316c2c => {
                edx = m.read32(edi);
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(edx);
                call!(0x3d7b0);
                pc = 0x316c3c; continue;
            }
            0x316c3c => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79460);
                pc = 0x316c45; continue;
            }
            0x316c45 => {
                push!(0x4);
                push!(0x1);
                push!(0x1);
                call!(0x78320);
                pc = 0x316c50; continue;
            }
            0x316c50 => {
                eax = m.read32(edi.wrapping_add(0x8));
                esp = esp.wrapping_add(0x20);
                esi = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x316c8f; continue; }
                pc = 0x316c5c; continue;
            }
            0x316c5c => {
                pc = 0x316c60; continue;
            }
            0x316c60 => {
                eax = m.read32(edi.wrapping_add(0x4));
                ecx = m.read32(eax.wrapping_add(esi.wrapping_mul(0x4)));
                push!(0x3165c0);
                push!(0x316500);
                push!(0x41400000);
                push!(0x4119999a);
                push!(0);
                push!(ecx);
                push!(0);
                call!(0x792b0);
                pc = 0x316c84; continue;
            }
            0x316c84 => {
                eax = m.read32(edi.wrapping_add(0x8));
                esp = esp.wrapping_add(0x1c);
                esi = esi.wrapping_add(1);
                fa = esi;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x316c60; continue; }
                pc = 0x316c8f; continue;
            }
            0x316c8f => {
                call!(0x78380);
                pc = 0x316c94; continue;
            }
            0x316c94 => {
                call!(0x783d0);
                pc = 0x316c99; continue;
            }
            0x316c99 => {
                push!(0);
                call!(0x795b0);
                pc = 0x316ca0; continue;
            }
            0x316ca0 => {
                esp = esp.wrapping_add(0x4);
                ebx = ebx.wrapping_add(0xc);
                ebp = ebp.wrapping_sub(1);
                fa = ebp;
                if ((fa != 0)) { pc = 0x316c10; continue; }
                pc = 0x316cad; continue;
            }
            0x316cad => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x316cb0; continue;
            }
            0x316cb0 => {
                call!(0x781a0);
                pc = 0x316cb5; continue;
            }
            0x316cb5 => {
                call!(0x22b90);
                pc = 0x316cba; continue;
            }
            0x316cba => {
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
                pc = 0x316d03; continue;
            }
            0x316d03 => {
                esp = esp.wrapping_add(0x8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                tail_to!(0x75e70); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x003179D0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3179d0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3179d0;
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
        { crate::pose::cover(0x3179d0, previous, pc); previous = pc; }
        match pc {
            0x3179d0 => {
                push!(esi);
                push!(0x2fb090);
                push!(0);
                push!(0x425a6667);
                push!(0);
                push!(0x840c);
                call!(0x75b70);
                pc = 0x3179e9; continue;
            }
            0x3179e9 => {
                push!(0x1);
                push!(0x1);
                push!(0);
                push!(0);
                call!(0x71b10);
                pc = 0x3179f6; continue;
            }
            0x3179f6 => {
                push!(0x28000);
                call!(0x71c50);
                pc = 0x317a00; continue;
            }
            0x317a00 => {
                esp = esp.wrapping_add(0x28);
                push!(0);
                push!(0);
                push!(0);
                call!(0x22b90);
                pc = 0x317a0e; continue;
            }
            0x317a0e => {
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
                pc = 0x317a5c; continue;
            }
            0x317a5c => {
                eax = m.read32(esp.wrapping_add(0x18));
                ecx = m.read32(eax.wrapping_add(0x4));
                esp = esp.wrapping_add(0x10);
                fa = ecx;
                fb = ((((0x52de1ci32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x317a8e; continue; }
                pc = 0x317a6e; continue;
            }
            0x317a6e => {
                esi = 0x52dd80;
                pc = 0x317a73; continue;
            }
            0x317a73 => {
                push!(0x316fe0);
                push!(esi);
                call!(0x3177f0);
                pc = 0x317a7e; continue;
            }
            0x317a7e => {
                esi = esi.wrapping_add(0xc);
                esp = esp.wrapping_add(0x8);
                fa = esi;
                fb = ((((0x52ddc8i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x317a73; continue; }
                pc = 0x317a8c; continue;
            }
            0x317a8c => {
                pc = 0x317adc; continue;
            }
            0x317a8e => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x200));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x317abe; continue; }
                pc = 0x317a9e; continue;
            }
            0x317a9e => {
                esi = 0x52dc40;
                pc = 0x317aa3; continue;
            }
            0x317aa3 => {
                push!(0x316fe0);
                push!(esi);
                call!(0x3177f0);
                pc = 0x317aae; continue;
            }
            0x317aae => {
                esi = esi.wrapping_add(0xc);
                esp = esp.wrapping_add(0x8);
                fa = esi;
                fb = ((((0x52dd0ci32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x317aa3; continue; }
                pc = 0x317abc; continue;
            }
            0x317abc => {
                pc = 0x317adc; continue;
            }
            0x317abe => {
                esi = 0x52dd10;
                pc = 0x317ac3; continue;
            }
            0x317ac3 => {
                push!(0x316fe0);
                push!(esi);
                call!(0x3177f0);
                pc = 0x317ace; continue;
            }
            0x317ace => {
                esi = esi.wrapping_add(0xc);
                esp = esp.wrapping_add(0x8);
                fa = esi;
                fb = ((((0x52dd7ci32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x317ac3; continue; }
                pc = 0x317adc; continue;
            }
            0x317adc => {
                call!(0x781a0);
                pc = 0x317ae1; continue;
            }
            0x317ae1 => {
                call!(0x22b90);
                pc = 0x317ae6; continue;
            }
            0x317ae6 => {
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
                pc = 0x317b2f; continue;
            }
            0x317b2f => {
                esp = esp.wrapping_add(0x8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                tail_to!(0x75e70); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00317D70: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f317d70<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x317d70;
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
        { crate::pose::cover(0x317d70, previous, pc); previous = pc; }
        match pc {
            0x317d70 => {
                esp = esp.wrapping_sub(0x14);
                eax = m.read32(0x7844a8);
                push!(ebx);
                ebx = m.read32(eax.wrapping_add(0x2f1c));
                push!(ebp);
                push!(esi);
                push!(edi);
                edi = (edi | 0xffffffffu32);
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ebp = edi;
                m.write32(esp.wrapping_add(0x10), ebp);
                if (fa == fb) { pc = 0x317d99; continue; }
                pc = 0x317d8f; continue;
            }
            0x317d8f => {
                fa = m.read32(ebx);
                fb = ((((0x43i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x317d99; continue; }
                pc = 0x317d94; continue;
            }
            0x317d94 => {
                esi = ebx.wrapping_add(0x18);
                pc = 0x317da0; continue;
            }
            0x317d99 => {
                call!(0x322260);
                pc = 0x317d9e; continue;
            }
            0x317d9e => {
                esi = eax;
                pc = 0x317da0; continue;
            }
            0x317da0 => {
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x317dc9; continue; }
                pc = 0x317da4; continue;
            }
            0x317da4 => {
                fa = m.read32(ebx);
                fb = ((((0x43i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x317dc9; continue; }
                pc = 0x317da9; continue;
            }
            0x317da9 => {
                m.write32(0x783b8c, ebx);
                fa = m.read32(ebx);
                fb = ((((0x43i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x317dc9; continue; }
                pc = 0x317db4; continue;
            }
            0x317db4 => {
                ecx = m.read32(ebx.wrapping_add(0x4));
                push!(ecx);
                call!(0x321e10);
                pc = 0x317dbd; continue;
            }
            0x317dbd => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x317dc9; continue; }
                pc = 0x317dc4; continue;
            }
            0x317dc4 => {
                call!(0x325f60);
                pc = 0x317dc9; continue;
            }
            0x317dc9 => {
                call!(0x322200);
                pc = 0x317dce; continue;
            }
            0x317dce => {
                fa = eax;
                fb = edi;
                if (fa == fb) { pc = 0x317e58; continue; }
                pc = 0x317dd6; continue;
            }
            0x317dd6 => {
                push!(eax);
                call!(0x322190);
                pc = 0x317ddc; continue;
            }
            0x317ddc => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = edi;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x10), eax);
                if (fas <= fbs) { pc = 0x317e1b; continue; }
                pc = 0x317de7; continue;
            }
            0x317de7 => {
                ecx = m.read32(0x7844a8);
                eax = ((eax as i32).wrapping_mul((0x220) as i32)) as u32;
                edx = m.read32(eax.wrapping_add(ecx).wrapping_add(0x4270));
                eax = eax.wrapping_add(ecx).wrapping_add(0x41e4);
                edx = ((edx as i32).wrapping_mul((0xb4) as i32)) as u32;
                ecx = edx.wrapping_add(ecx).wrapping_add(0x81c8);
                m.write32(esp.wrapping_add(0x14), ecx);
                fa = m.read32(eax.wrapping_add(0x88));
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x317e2c; continue; }
                pc = 0x317e1b; continue;
            }
            0x317e1b => {
                push!(0xb15);
                call!(0x3d7b0);
                pc = 0x317e25; continue;
            }
            0x317e25 => {
                esp = esp.wrapping_add(0x4);
                m.write32(esp.wrapping_add(0x14), eax);
                pc = 0x317e2c; continue;
            }
            0x317e2c => {
                ebp = m.read32(esp.wrapping_add(0x10));
                pc = 0x317e30; continue;
            }
            0x317e30 => {
                push!(ebp);
                call!(0x321f20);
                pc = 0x317e36; continue;
            }
            0x317e36 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x317e6b; continue; }
                pc = 0x317e3d; continue;
            }
            0x317e3d => {
                call!(0x321e60);
                pc = 0x317e42; continue;
            }
            0x317e42 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x317e6b; continue; }
                pc = 0x317e46; continue;
            }
            0x317e46 => {
                m.write32(esp.wrapping_add(0x1c), 0);
                m.write32(esp.wrapping_add(0x18), 0x326280);
                pc = 0x317e88; continue;
            }
            0x317e58 => {
                push!(0xb15);
                call!(0x3d7b0);
                pc = 0x317e62; continue;
            }
            0x317e62 => {
                esp = esp.wrapping_add(0x4);
                m.write32(esp.wrapping_add(0x14), eax);
                pc = 0x317e30; continue;
            }
            0x317e6b => {
                call!(0x321e60);
                pc = 0x317e70; continue;
            }
            0x317e70 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0x1c), edi);
                m.write32(esp.wrapping_add(0x18), 0x321e80);
                if (fa == fb) { pc = 0x317e88; continue; }
                pc = 0x317e80; continue;
            }
            0x317e80 => {
                m.write32(esp.wrapping_add(0x18), 0x317d30);
                pc = 0x317e88; continue;
            }
            0x317e88 => {
                push!(ebp);
                call!(0x321ef0);
                pc = 0x317e8e; continue;
            }
            0x317e8e => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x317eba; continue; }
                pc = 0x317e95; continue;
            }
            0x317e95 => {
                edx = m.read32(0x7844a8);
                ebp = ((ebp as i32).wrapping_mul((0x220) as i32)) as u32;
                fa = m.read32(edx.wrapping_add(ebp).wrapping_add(0x4234));
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x317eba; continue; }
                pc = 0x317eab; continue;
            }
            0x317eab => {
                m.write32(esp.wrapping_add(0x20), 0x2);
                ebx = 0x326280;
                pc = 0x317ec3; continue;
            }
            0x317eba => {
                m.write32(esp.wrapping_add(0x20), edi);
                ebx = 0x317d50;
                pc = 0x317ec3; continue;
            }
            0x317ec3 => {
                push!(ebx);
                push!(0);
                push!(0x42480000);
                push!(0);
                push!(0xa400);
                call!(0x75b70);
                pc = 0x317ed7; continue;
            }
            0x317ed7 => {
                ebp = m.read32(esp.wrapping_add(0x3c));
                push!(0x1);
                push!(0x1);
                push!(0x3f000000);
                push!(0x1);
                m.write32(ebp.wrapping_add(0x218), 0x1);
                call!(0x71b10);
                pc = 0x317ef5; continue;
            }
            0x317ef5 => {
                push!(0x20000);
                call!(0x71c50);
                pc = 0x317eff; continue;
            }
            0x317eff => {
                esp = esp.wrapping_add(0x28);
                push!(0);
                push!(0);
                push!(0);
                call!(0x22b90);
                pc = 0x317f0d; continue;
            }
            0x317f0d => {
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
                pc = 0x317f5b; continue;
            }
            0x317f5b => {
                esp = esp.wrapping_add(0x10);
                push!(0);
                push!(0);
                push!(0);
                push!(edi);
                push!(0xb1e);
                call!(0x3d7b0);
                pc = 0x317f6f; continue;
            }
            0x317f6f => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x317f78; continue;
            }
            0x317f78 => {
                push!(0);
                push!(0x800);
                call!(0x7d4a0);
                pc = 0x317f84; continue;
            }
            0x317f84 => {
                push!(0x86);
                call!(0x716d0);
                pc = 0x317f8e; continue;
            }
            0x317f8e => {
                esp = esp.wrapping_add(0x20);
                push!(0);
                push!(0xb1f);
                call!(0x3d7b0);
                pc = 0x317f9d; continue;
            }
            0x317f9d => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x317fa6; continue;
            }
            0x317fa6 => {
                esp = esp.wrapping_add(0x8);
                push!(0x1);
                push!(0xb20);
                call!(0x3d7b0);
                pc = 0x317fb5; continue;
            }
            0x317fb5 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x317fbe; continue;
            }
            0x317fbe => {
                push!(esi);
                call!(0x78590);
                pc = 0x317fc4; continue;
            }
            0x317fc4 => {
                esp = esp.wrapping_add(0xc);
                call!(0x783d0);
                pc = 0x317fcc; continue;
            }
            0x317fcc => {
                push!(0);
                push!(0);
                push!(0);
                push!(edi);
                push!(0xb1b);
                call!(0x3d7b0);
                pc = 0x317fdd; continue;
            }
            0x317fdd => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x317fe6; continue;
            }
            0x317fe6 => {
                push!(0);
                push!(0x800);
                call!(0x7d4a0);
                pc = 0x317ff2; continue;
            }
            0x317ff2 => {
                push!(0x85);
                call!(0x716d0);
                pc = 0x317ffc; continue;
            }
            0x317ffc => {
                esp = esp.wrapping_add(0x20);
                push!(0);
                push!(0xb1c);
                call!(0x3d7b0);
                pc = 0x31800b; continue;
            }
            0x31800b => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x318014; continue;
            }
            0x318014 => {
                esp = esp.wrapping_add(0x8);
                push!(0x1);
                push!(0xb1d);
                call!(0x3d7b0);
                pc = 0x318023; continue;
            }
            0x318023 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x31802c; continue;
            }
            0x31802c => {
                eax = esi.wrapping_add(0x4);
                push!(eax);
                call!(0x78590);
                pc = 0x318035; continue;
            }
            0x318035 => {
                esp = esp.wrapping_add(0xc);
                call!(0x783d0);
                pc = 0x31803d; continue;
            }
            0x31803d => {
                eax = m.read32(esi.wrapping_add(0x8));
                edi = esi.wrapping_add(0x8);
                ecx = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ecx = (ecx & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                push!(0);
                push!(0);
                push!(ecx);
                push!(0xffffffffu32);
                push!(0xb19);
                call!(0x3d7b0);
                pc = 0x31805b; continue;
            }
            0x31805b => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x318064; continue;
            }
            0x318064 => {
                push!(0x20);
                push!(0);
                push!(0x1);
                push!(0x3c);
                push!(0);
                edx = esi.wrapping_add(0x10);
                push!(edx);
                push!(0);
                push!(0);
                call!(0x7d5e0);
                pc = 0x31807b; continue;
            }
            0x31807b => {
                push!(0x83);
                call!(0x716d0);
                pc = 0x318085; continue;
            }
            0x318085 => {
                esp = esp.wrapping_add(0x38);
                push!(0);
                push!(0);
                push!(0);
                push!(0x800);
                push!(0xffffffffu32);
                push!(0xb17);
                call!(0x3d7b0);
                pc = 0x31809f; continue;
            }
            0x31809f => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x3180a8; continue;
            }
            0x3180a8 => {
                push!(0x84);
                call!(0x716d0);
                pc = 0x3180b2; continue;
            }
            0x3180b2 => {
                push!(0);
                push!(0);
                call!(0x7d4f0);
                pc = 0x3180bb; continue;
            }
            0x3180bb => {
                push!(edi);
                call!(0x78560);
                pc = 0x3180c1; continue;
            }
            0x3180c1 => {
                esp = esp.wrapping_add(0x28);
                call!(0x783d0);
                pc = 0x3180c9; continue;
            }
            0x3180c9 => {
                ecx = m.read32(esi.wrapping_add(0xc));
                edi = esi.wrapping_add(0xc);
                eax = 0;
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = (eax & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                push!(0);
                push!(0);
                push!(eax);
                push!(0xffffffffu32);
                push!(0xb1a);
                call!(0x3d7b0);
                pc = 0x3180e7; continue;
            }
            0x3180e7 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x3180f0; continue;
            }
            0x3180f0 => {
                push!(0x20);
                push!(0);
                push!(0x1);
                push!(0x3c);
                push!(0);
                esi = esi.wrapping_add(0x14);
                push!(esi);
                push!(0);
                push!(0);
                call!(0x7d5e0);
                pc = 0x318107; continue;
            }
            0x318107 => {
                push!(0x83);
                call!(0x716d0);
                pc = 0x318111; continue;
            }
            0x318111 => {
                esp = esp.wrapping_add(0x38);
                push!(0);
                push!(0);
                push!(0);
                push!(0x800);
                push!(0xffffffffu32);
                push!(0xb18);
                call!(0x3d7b0);
                pc = 0x31812b; continue;
            }
            0x31812b => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x318134; continue;
            }
            0x318134 => {
                push!(0x84);
                call!(0x716d0);
                pc = 0x31813e; continue;
            }
            0x31813e => {
                push!(0);
                push!(0);
                call!(0x7d4f0);
                pc = 0x318147; continue;
            }
            0x318147 => {
                push!(edi);
                call!(0x78560);
                pc = 0x31814d; continue;
            }
            0x31814d => {
                esp = esp.wrapping_add(0x28);
                call!(0x783d0);
                pc = 0x318155; continue;
            }
            0x318155 => {
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0xb16);
                call!(0x3d7b0);
                pc = 0x318167; continue;
            }
            0x318167 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79460);
                pc = 0x318170; continue;
            }
            0x318170 => {
                ecx = m.read32(esp.wrapping_add(0x28));
                push!(0);
                push!(0);
                push!(0);
                push!(0x100800);
                push!(0xffffffffu32);
                push!(ecx);
                call!(0x79d60);
                pc = 0x318187; continue;
            }
            0x318187 => {
                push!(0x80);
                call!(0x716d0);
                pc = 0x318191; continue;
            }
            0x318191 => {
                esp = esp.wrapping_add(0x30);
                call!(0x783d0);
                pc = 0x318199; continue;
            }
            0x318199 => {
                edx = m.read32(esp.wrapping_add(0x18));
                eax = m.read32(esp.wrapping_add(0x1c));
                push!(0);
                push!(edx);
                push!(0);
                push!(0x800);
                push!(eax);
                push!(0xb21);
                call!(0x3d7b0);
                pc = 0x3181b6; continue;
            }
            0x3181b6 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x3181bf; continue;
            }
            0x3181bf => {
                push!(0x81);
                call!(0x716d0);
                pc = 0x3181c9; continue;
            }
            0x3181c9 => {
                ecx = m.read32(esp.wrapping_add(0x2c));
                esp = esp.wrapping_add(0x1c);
                push!(0);
                push!(0x326280);
                push!(0);
                push!(ecx);
                call!(0x321f20);
                pc = 0x3181df; continue;
            }
            0x3181df => {
                esp = esp.wrapping_add(0x4);
                cf = (((((eax) != 0))) as i32);
                eax = (((((eax) as i32).wrapping_neg())) as u32);
                eax = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                eax = (eax & 0x7ff);
                eax = eax.wrapping_add(1);
                push!(eax);
                push!(0x1);
                push!(0xa23);
                call!(0x3d7b0);
                pc = 0x3181f9; continue;
            }
            0x3181f9 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x318202; continue;
            }
            0x318202 => {
                push!(0x82);
                call!(0x716d0);
                pc = 0x31820c; continue;
            }
            0x31820c => {
                push!(0x40000000);
                call!(0x784a0);
                pc = 0x318216; continue;
            }
            0x318216 => {
                edx = m.read32(esp.wrapping_add(0x40));
                esp = esp.wrapping_add(0x20);
                push!(0);
                push!(ebx);
                push!(0x40000000);
                push!(0x800);
                push!(edx);
                push!(0xb5e);
                call!(0x3d7b0);
                pc = 0x318235; continue;
            }
            0x318235 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x31823e; continue;
            }
            0x31823e => {
                call!(0x781a0);
                pc = 0x318243; continue;
            }
            0x318243 => {
                xmm0 = Xmm::scalar(m.read32(0x49f7e8));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ebp.wrapping_add(0x68)))).to_bits();
                eax = m.read32(0x7bb1e8);
                esp = esp.wrapping_add(0x14);
                m.write32(esp, xmm0.lanes[0]);
                push!(eax);
                call!(0x77320);
                pc = 0x318263; continue;
            }
            0x318263 => {
                esp = esp.wrapping_add(0x8);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x14);
                tail_to!(0x75e70); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00318350: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f318350<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x318350;
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
        { crate::pose::cover(0x318350, previous, pc); previous = pc; }
        match pc {
            0x318350 => {
                eax = m.read32(0x7844a8);
                push!(ebx);
                push!(ebp);
                push!(esi);
                esi = m.read32(eax.wrapping_add(0x2f1c));
                push!(edi);
                esi = esi.wrapping_add(0x18);
                eax = m.read32(esi.wrapping_add(0x14));
                edi = esi.wrapping_add(0x14);
                ecx = eax;
                ecx = ecx.wrapping_shr((((((((0x3i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx & 0x1);
                m.write32(esi.wrapping_add(0x1c), ecx);
                edx = m.read32(0x7844a8);
                ecx = m.read32(edx.wrapping_add(0x2f1c));
                ebp = esi.wrapping_add(0x1c);
                edx = 0;
                fa = m.read32(ecx);
                fb = ((((0x36i32)) as u32) & 0xffffffffu32);
                push!(0x2fb3c0);
                edx = (edx & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                push!(0);
                push!(0x42480000);
                eax = (eax & 0xfffffff7u32);
                push!(0);
                ebx = esi.wrapping_add(0x18);
                push!(0x8400);
                m.write32(ebx, edx);
                m.write32(edi, eax);
                call!(0x75b70);
                pc = 0x3183ac; continue;
            }
            0x3183ac => {
                eax = m.read32(esp.wrapping_add(0x28));
                push!(0x1);
                push!(0x1);
                push!(0x3f000000);
                push!(0x1);
                m.write32(eax.wrapping_add(0x218), 0x1);
                call!(0x71b10);
                pc = 0x3183ca; continue;
            }
            0x3183ca => {
                push!(0x20000);
                call!(0x71c50);
                pc = 0x3183d4; continue;
            }
            0x3183d4 => {
                esp = esp.wrapping_add(0x28);
                push!(0);
                push!(0);
                push!(0);
                call!(0x22b90);
                pc = 0x3183e2; continue;
            }
            0x3183e2 => {
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
                pc = 0x318430; continue;
            }
            0x318430 => {
                esp = esp.wrapping_add(0x10);
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0xafe);
                call!(0x3d7b0);
                pc = 0x318445; continue;
            }
            0x318445 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x31844e; continue;
            }
            0x31844e => {
                push!(0);
                push!(0);
                call!(0x7d4a0);
                pc = 0x318457; continue;
            }
            0x318457 => {
                push!(0x8a);
                call!(0x716d0);
                pc = 0x318461; continue;
            }
            0x318461 => {
                esp = esp.wrapping_add(0x20);
                push!(0x4);
                push!(0x9e8);
                call!(0x3d7b0);
                pc = 0x318470; continue;
            }
            0x318470 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x318479; continue;
            }
            0x318479 => {
                esp = esp.wrapping_add(0x8);
                push!(0x84);
                push!(0x9e9);
                call!(0x3d7b0);
                pc = 0x31848b; continue;
            }
            0x31848b => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x318494; continue;
            }
            0x318494 => {
                push!(edi);
                call!(0x78590);
                pc = 0x31849a; continue;
            }
            0x31849a => {
                esp = esp.wrapping_add(0xc);
                call!(0x783d0);
                pc = 0x3184a2; continue;
            }
            0x3184a2 => {
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0x9ec);
                call!(0x3d7b0);
                pc = 0x3184b4; continue;
            }
            0x3184b4 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x3184bd; continue;
            }
            0x3184bd => {
                push!(0);
                push!(0);
                call!(0x7d4a0);
                pc = 0x3184c6; continue;
            }
            0x3184c6 => {
                push!(0x8b);
                call!(0x716d0);
                pc = 0x3184d0; continue;
            }
            0x3184d0 => {
                esp = esp.wrapping_add(0x20);
                push!(0);
                push!(0x9ee);
                call!(0x3d7b0);
                pc = 0x3184df; continue;
            }
            0x3184df => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x3184e8; continue;
            }
            0x3184e8 => {
                esp = esp.wrapping_add(0x8);
                push!(0x1);
                push!(0x72e);
                call!(0x3d7b0);
                pc = 0x3184f7; continue;
            }
            0x3184f7 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x318500; continue;
            }
            0x318500 => {
                esp = esp.wrapping_add(0x8);
                push!(0x2);
                push!(0x72f);
                call!(0x3d7b0);
                pc = 0x31850f; continue;
            }
            0x31850f => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x318518; continue;
            }
            0x318518 => {
                esp = esp.wrapping_add(0x8);
                push!(0x3);
                push!(0x730);
                call!(0x3d7b0);
                pc = 0x318527; continue;
            }
            0x318527 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x318530; continue;
            }
            0x318530 => {
                esp = esp.wrapping_add(0x8);
                push!(0x4);
                push!(0x731);
                call!(0x3d7b0);
                pc = 0x31853f; continue;
            }
            0x31853f => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x318548; continue;
            }
            0x318548 => {
                ecx = esi.wrapping_add(0xc);
                push!(ecx);
                call!(0x78590);
                pc = 0x318551; continue;
            }
            0x318551 => {
                esp = esp.wrapping_add(0xc);
                call!(0x783d0);
                pc = 0x318559; continue;
            }
            0x318559 => {
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0x9ed);
                call!(0x3d7b0);
                pc = 0x31856b; continue;
            }
            0x31856b => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x318574; continue;
            }
            0x318574 => {
                push!(0);
                push!(0);
                call!(0x7d4a0);
                pc = 0x31857d; continue;
            }
            0x31857d => {
                push!(0x8c);
                call!(0x716d0);
                pc = 0x318587; continue;
            }
            0x318587 => {
                esp = esp.wrapping_add(0x20);
                push!(0);
                push!(0x9ee);
                call!(0x3d7b0);
                pc = 0x318596; continue;
            }
            0x318596 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x31859f; continue;
            }
            0x31859f => {
                esp = esp.wrapping_add(0x8);
                push!(0x1);
                push!(0x732);
                call!(0x3d7b0);
                pc = 0x3185ae; continue;
            }
            0x3185ae => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x3185b7; continue;
            }
            0x3185b7 => {
                esp = esp.wrapping_add(0x8);
                push!(0x2);
                push!(0x733);
                call!(0x3d7b0);
                pc = 0x3185c6; continue;
            }
            0x3185c6 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x3185cf; continue;
            }
            0x3185cf => {
                edx = esi.wrapping_add(0x10);
                push!(edx);
                call!(0x78590);
                pc = 0x3185d8; continue;
            }
            0x3185d8 => {
                esp = esp.wrapping_add(0xc);
                call!(0x783d0);
                pc = 0x3185e0; continue;
            }
            0x3185e0 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f1c));
                fa = m.read32(ecx);
                fb = ((((0x37i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x318759; continue; }
                pc = 0x3185f4; continue;
            }
            0x3185f4 => {
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0xb0e);
                call!(0x3d7b0);
                pc = 0x318606; continue;
            }
            0x318606 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x31860f; continue;
            }
            0x31860f => {
                push!(0);
                push!(0);
                call!(0x7d4a0);
                pc = 0x318618; continue;
            }
            0x318618 => {
                push!(0x8d);
                call!(0x716d0);
                pc = 0x318622; continue;
            }
            0x318622 => {
                esp = esp.wrapping_add(0x20);
                push!(0x2a);
                push!(0xb22);
                call!(0x3d7b0);
                pc = 0x318631; continue;
            }
            0x318631 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x31863a; continue;
            }
            0x31863a => {
                esp = esp.wrapping_add(0x8);
                push!(0x40);
                push!(0xb23);
                call!(0x3d7b0);
                pc = 0x318649; continue;
            }
            0x318649 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x318652; continue;
            }
            0x318652 => {
                esp = esp.wrapping_add(0x8);
                push!(0x34);
                push!(0xb24);
                call!(0x3d7b0);
                pc = 0x318661; continue;
            }
            0x318661 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x31866a; continue;
            }
            0x31866a => {
                esp = esp.wrapping_add(0x8);
                push!(0x10);
                push!(0xb25);
                call!(0x3d7b0);
                pc = 0x318679; continue;
            }
            0x318679 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x318682; continue;
            }
            0x318682 => {
                esp = esp.wrapping_add(0x8);
                push!(0x3a);
                push!(0x175c);
                call!(0x3d7b0);
                pc = 0x318691; continue;
            }
            0x318691 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x31869a; continue;
            }
            0x31869a => {
                esp = esp.wrapping_add(0x8);
                push!(0x4);
                push!(0x1764);
                call!(0x3d7b0);
                pc = 0x3186a9; continue;
            }
            0x3186a9 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x3186b2; continue;
            }
            0x3186b2 => {
                esp = esp.wrapping_add(0x8);
                push!(0x26);
                push!(0x1769);
                call!(0x3d7b0);
                pc = 0x3186c1; continue;
            }
            0x3186c1 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x3186ca; continue;
            }
            0x3186ca => {
                esp = esp.wrapping_add(0x8);
                push!(0xb);
                push!(0x1768);
                call!(0x3d7b0);
                pc = 0x3186d9; continue;
            }
            0x3186d9 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x3186e2; continue;
            }
            0x3186e2 => {
                edx = esi.wrapping_add(0x8);
                push!(edx);
                call!(0x78590);
                pc = 0x3186eb; continue;
            }
            0x3186eb => {
                esp = esp.wrapping_add(0xc);
                call!(0x783d0);
                pc = 0x3186f3; continue;
            }
            0x3186f3 => {
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0xb83);
                call!(0x3d7b0);
                pc = 0x318705; continue;
            }
            0x318705 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x31870e; continue;
            }
            0x31870e => {
                push!(0);
                push!(0);
                call!(0x7d4d0);
                pc = 0x318717; continue;
            }
            0x318717 => {
                push!(0x9ef);
                call!(0x3d7b0);
                pc = 0x318721; continue;
            }
            0x318721 => {
                push!(eax);
                call!(0x717d0);
                pc = 0x318727; continue;
            }
            0x318727 => {
                push!(0x8e);
                call!(0x716d0);
                pc = 0x318731; continue;
            }
            0x318731 => {
                push!(0x9f0);
                call!(0x3d7b0);
                pc = 0x31873b; continue;
            }
            0x31873b => {
                push!(eax);
                call!(0x717d0);
                pc = 0x318741; continue;
            }
            0x318741 => {
                push!(0x8f);
                call!(0x716d0);
                pc = 0x31874b; continue;
            }
            0x31874b => {
                push!(ebx);
                call!(0x78560);
                pc = 0x318751; continue;
            }
            0x318751 => {
                esp = esp.wrapping_add(0x38);
                call!(0x783d0);
                pc = 0x318759; continue;
            }
            0x318759 => {
                push!(0);
                push!(0);
                push!(0);
                push!(0x800);
                push!(0xffffffffu32);
                push!(0x9ea);
                call!(0x3d7b0);
                pc = 0x318770; continue;
            }
            0x318770 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x318779; continue;
            }
            0x318779 => {
                push!(0);
                push!(0);
                call!(0x7d4f0);
                pc = 0x318782; continue;
            }
            0x318782 => {
                push!(0x90);
                call!(0x716d0);
                pc = 0x31878c; continue;
            }
            0x31878c => {
                push!(ebp);
                call!(0x78560);
                pc = 0x318792; continue;
            }
            0x318792 => {
                esp = esp.wrapping_add(0x28);
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0xb0a);
                call!(0x3d7b0);
                pc = 0x3187a7; continue;
            }
            0x3187a7 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x3187b0; continue;
            }
            0x3187b0 => {
                push!(0x9c9);
                call!(0x3d7b0);
                pc = 0x3187ba; continue;
            }
            0x3187ba => {
                esp = esp.wrapping_add(0x18);
                push!(eax);
                push!(0x9c8);
                call!(0x3d7b0);
                pc = 0x3187c8; continue;
            }
            0x3187c8 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0);
                push!(0);
                push!(0);
                call!(0x7d680);
                pc = 0x3187d7; continue;
            }
            0x3187d7 => {
                push!(0x91);
                call!(0x716d0);
                pc = 0x3187e1; continue;
            }
            0x3187e1 => {
                call!(0x71630);
                pc = 0x3187e6; continue;
            }
            0x3187e6 => {
                push!(eax);
                push!(esi);
                call!(0x785c0);
                pc = 0x3187ed; continue;
            }
            0x3187ed => {
                esp = esp.wrapping_add(0x20);
                call!(0x783d0);
                pc = 0x3187f5; continue;
            }
            0x3187f5 => {
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0xb0b);
                call!(0x3d7b0);
                pc = 0x318807; continue;
            }
            0x318807 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x318810; continue;
            }
            0x318810 => {
                push!(0xb0d);
                call!(0x3d7b0);
                pc = 0x31881a; continue;
            }
            0x31881a => {
                esp = esp.wrapping_add(0x18);
                push!(eax);
                push!(0xb0c);
                call!(0x3d7b0);
                pc = 0x318828; continue;
            }
            0x318828 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0);
                push!(0);
                push!(0);
                call!(0x7d680);
                pc = 0x318837; continue;
            }
            0x318837 => {
                push!(0x92);
                call!(0x716d0);
                pc = 0x318841; continue;
            }
            0x318841 => {
                call!(0x71630);
                pc = 0x318846; continue;
            }
            0x318846 => {
                push!(eax);
                esi = esi.wrapping_add(0x4);
                push!(esi);
                call!(0x785c0);
                pc = 0x318850; continue;
            }
            0x318850 => {
                call!(0x783d0);
                pc = 0x318855; continue;
            }
            0x318855 => {
                push!(0x40000000);
                call!(0x784a0);
                pc = 0x31885f; continue;
            }
            0x31885f => {
                esp = esp.wrapping_add(0x24);
                push!(0);
                push!(0x2fb3c0);
                push!(0x40000000);
                push!(0);
                push!(0xffffffffu32);
                push!(0xb5e);
                call!(0x3d7b0);
                pc = 0x31887c; continue;
            }
            0x31887c => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x318885; continue;
            }
            0x318885 => {
                call!(0x781a0);
                pc = 0x31888a; continue;
            }
            0x31888a => {
                eax = m.read32(esp.wrapping_add(0x2c));
                xmm0 = Xmm::scalar(m.read32(0x49f7e8));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x68)))).to_bits();
                ecx = m.read32(0x7bb1e8);
                esp = esp.wrapping_add(0x14);
                m.write32(esp, xmm0.lanes[0]);
                push!(ecx);
                call!(0x77320);
                pc = 0x3188af; continue;
            }
            0x3188af => {
                esp = esp.wrapping_add(0x8);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                tail_to!(0x75e70); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x003188C0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3188c0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3188c0;
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
        { crate::pose::cover(0x3188c0, previous, pc); previous = pc; }
        match pc {
            0x3188c0 => {
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
                pc = 0x3188e8; continue;
            }
            0x3188e8 => {
                edi = m.read32(esp.wrapping_add(0x20));
                push!(0x1);
                push!(0x1);
                push!(0x3f000000);
                push!(0x1);
                m.write32(edi.wrapping_add(0x218), 0x1);
                call!(0x71b10);
                pc = 0x318906; continue;
            }
            0x318906 => {
                push!(0x20000);
                call!(0x71c50);
                pc = 0x318910; continue;
            }
            0x318910 => {
                esp = esp.wrapping_add(0x28);
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0xb0b);
                call!(0x3d7b0);
                pc = 0x318925; continue;
            }
            0x318925 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x31892e; continue;
            }
            0x31892e => {
                push!(0xb0d);
                call!(0x3d7b0);
                pc = 0x318938; continue;
            }
            0x318938 => {
                esp = esp.wrapping_add(0x18);
                push!(eax);
                push!(0xb0c);
                call!(0x3d7b0);
                pc = 0x318946; continue;
            }
            0x318946 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0);
                push!(0);
                push!(0);
                call!(0x7d680);
                pc = 0x318955; continue;
            }
            0x318955 => {
                push!(0x93);
                call!(0x716d0);
                pc = 0x31895f; continue;
            }
            0x31895f => {
                call!(0x71630);
                pc = 0x318964; continue;
            }
            0x318964 => {
                push!(eax);
                push!(esi);
                call!(0x785c0);
                pc = 0x31896b; continue;
            }
            0x31896b => {
                esp = esp.wrapping_add(0x20);
                call!(0x783d0);
                pc = 0x318973; continue;
            }
            0x318973 => {
                push!(0);
                push!(0);
                push!(0x8000800);
                push!(0xffffffffu32);
                push!(0xb0e);
                call!(0x3d7b0);
                pc = 0x318988; continue;
            }
            0x318988 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x318991; continue;
            }
            0x318991 => {
                push!(0);
                push!(0);
                call!(0x7d4a0);
                pc = 0x31899a; continue;
            }
            0x31899a => {
                push!(0x94);
                call!(0x716d0);
                pc = 0x3189a4; continue;
            }
            0x3189a4 => {
                ecx = m.read32(0x7844a8);
                edx = m.read32(ecx.wrapping_add(0x2f1c));
                eax = m.read32(edx);
                esp = esp.wrapping_add(0x20);
                fa = eax;
                fb = ((((0x32i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3189f3; continue; }
                pc = 0x3189ba; continue;
            }
            0x3189ba => {
                push!(0x2a);
                push!(0xb22);
                call!(0x3d7b0);
                pc = 0x3189c6; continue;
            }
            0x3189c6 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x3189cf; continue;
            }
            0x3189cf => {
                esp = esp.wrapping_add(0x8);
                push!(0x40);
                push!(0xb23);
                call!(0x3d7b0);
                pc = 0x3189de; continue;
            }
            0x3189de => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x3189e7; continue;
            }
            0x3189e7 => {
                esp = esp.wrapping_add(0x8);
                push!(0x34);
                push!(0xb24);
                pc = 0x318a6d; continue;
            }
            0x3189f3 => {
                fa = eax;
                fb = ((((0x33i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x318a61; continue; }
                pc = 0x3189f8; continue;
            }
            0x3189f8 => {
                push!(0x10);
                push!(0xb25);
                call!(0x3d7b0);
                pc = 0x318a04; continue;
            }
            0x318a04 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x318a0d; continue;
            }
            0x318a0d => {
                esp = esp.wrapping_add(0x8);
                push!(0x3a);
                push!(0x175c);
                call!(0x3d7b0);
                pc = 0x318a1c; continue;
            }
            0x318a1c => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x318a25; continue;
            }
            0x318a25 => {
                esp = esp.wrapping_add(0x8);
                push!(0x16);
                push!(0xb26);
                call!(0x3d7b0);
                pc = 0x318a34; continue;
            }
            0x318a34 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x318a3d; continue;
            }
            0x318a3d => {
                esp = esp.wrapping_add(0x8);
                push!(0x18);
                push!(0x175e);
                call!(0x3d7b0);
                pc = 0x318a4c; continue;
            }
            0x318a4c => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x318a55; continue;
            }
            0x318a55 => {
                esp = esp.wrapping_add(0x8);
                push!(0x4);
                push!(0x1764);
                pc = 0x318a6d; continue;
            }
            0x318a61 => {
                fa = eax;
                fb = ((((0x34i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x318a7e; continue; }
                pc = 0x318a66; continue;
            }
            0x318a66 => {
                push!(0xd);
                push!(0xb29);
                pc = 0x318a6d; continue;
            }
            0x318a6d => {
                call!(0x3d7b0);
                pc = 0x318a72; continue;
            }
            0x318a72 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x318a7b; continue;
            }
            0x318a7b => {
                esp = esp.wrapping_add(0x8);
                pc = 0x318a7e; continue;
            }
            0x318a7e => {
                esi = esi.wrapping_add(0x4);
                push!(esi);
                call!(0x78590);
                pc = 0x318a87; continue;
            }
            0x318a87 => {
                call!(0x783d0);
                pc = 0x318a8c; continue;
            }
            0x318a8c => {
                push!(0x40000000);
                call!(0x784a0);
                pc = 0x318a96; continue;
            }
            0x318a96 => {
                esp = esp.wrapping_add(0x8);
                push!(0);
                push!(0x2fb3c0);
                push!(0x40000000);
                push!(0);
                push!(0xffffffffu32);
                push!(0xb5e);
                call!(0x3d7b0);
                pc = 0x318ab3; continue;
            }
            0x318ab3 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x318abc; continue;
            }
            0x318abc => {
                call!(0x75e70);
                pc = 0x318ac1; continue;
            }
            0x318ac1 => {
                xmm0 = Xmm::scalar(m.read32(0x49f7e8));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x68)))).to_bits();
                eax = m.read32(0x7bb1e8);
                esp = esp.wrapping_add(0x14);
                m.write32(esp, xmm0.lanes[0]);
                push!(eax);
                call!(0x77320);
                pc = 0x318ae1; continue;
            }
            0x318ae1 => {
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
