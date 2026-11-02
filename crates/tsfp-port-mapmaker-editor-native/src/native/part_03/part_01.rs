use super::*;
#[rustfmt::skip] mod unformatted { use super::*;
/// 0x003051F0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3051f0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3051f0;
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
        { crate::pose::cover(0x3051f0, previous, pc); previous = pc; }
        match pc {
            0x3051f0 => {
                eax = m.read32(0x7ba9e0);
                eax = ((eax as i32).wrapping_mul((0x1e0) as i32)) as u32;
                ecx = m.read32(eax.wrapping_add(0x7ba26c));
                push!(ebx);
                push!(ebp);
                push!(esi);
                push!(edi);
                push!(ecx);
                call!(0x77470);
                pc = 0x30520b; continue;
            }
            0x30520b => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                eax = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x4);
                edi = (edi | 0xffffffffu32);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x3053f4; continue; }
                pc = 0x305225; continue;
            }
            0x305225 => {
                edx = m.read32(eax.wrapping_add(0x25c));
                ecx = 0x2000;
                fa = ecx;
                fb = edx;
                if ((fa & fb) != 0) { pc = 0x3053f4; continue; }
                pc = 0x305238; continue;
            }
            0x305238 => {
                edx = m.read32(eax.wrapping_add(0x2f20));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x3053f4; continue; }
                pc = 0x305246; continue;
            }
            0x305246 => {
                m.write32(eax.wrapping_add(0x25c), (m.read32(eax.wrapping_add(0x25c)) | ecx));
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f18));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3053f4; continue; }
                pc = 0x30525f; continue;
            }
            0x30525f => {
                ecx = (ecx & 0xffff_0000) | ((((m.read16(eax.wrapping_add(0x2f10))) as u32)) & 0xffff);
                fa = ((((((ecx) as u16))) as u32) & 0xffffu32);
                fb = ((((0i32)) as u32) & 0xffffu32);
                fas = (((((fa)) as i16)) as i32);
                fbs = (((((fb)) as i16)) as i32);
                if (fas < fbs) { pc = 0x3053f4; continue; }
                pc = 0x30526f; continue;
            }
            0x30526f => {
                edx = m.read32(eax.wrapping_add(0x25c));
                fa = ((((((edx) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x3053f4; continue; }
                pc = 0x30527e; continue;
            }
            0x30527e => {
                fa = edx;
                fb = ((((0x410000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) != 0) { pc = 0x3053f4; continue; }
                pc = 0x30528a; continue;
            }
            0x30528a => {
                edx = ((((ecx) as u16)) as u16 as i16 as i32 as u32);
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0x238))) as u32)) & 0xff);
                edx = ((edx as i32).wrapping_mul((0xe4) as i32)) as u32;
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                esi = edx.wrapping_add(eax).wrapping_add(0x288);
                if ((fa & fb) != 0) { pc = 0x3052c3; continue; }
                pc = 0x3052a5; continue;
            }
            0x3052a5 => {
                push!(0xd);
                call!(0x3370a0);
                pc = 0x3052ac; continue;
            }
            0x3052ac => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x238));
                esp = esp.wrapping_add(0x4);
                ecx = (ecx | 0x20);
                m.write32(eax.wrapping_add(0x238), ecx);
                pc = 0x3052c3; continue;
            }
            0x3052c3 => {
                push!(0x4);
                call!(0x300670);
                pc = 0x3052ca; continue;
            }
            0x3052ca => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3053cf; continue; }
                pc = 0x3052d5; continue;
            }
            0x3052d5 => {
                ecx = m.read32(0x7844a8);
                eax = (((((m.read16(ecx.wrapping_add(0x2f12)) as i16)) as i32)) as u32);
                edx = m.read32(esi.wrapping_add(0x58));
                fa = edx;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x305322; continue; }
                pc = 0x3052e9; continue;
            }
            0x3052e9 => {
                eax = edx;
                eax = eax.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = eax.wrapping_add(esi).wrapping_add(0x64);
                pc = 0x3052f2; continue;
            }
            0x3052f2 => {
                ecx = eax.wrapping_add(((0x10i32.wrapping_neg()) as u32));
                ebp = m.read32(ecx);
                ebx = eax;
                m.write32(ebx, ebp);
                ebp = m.read32(ecx.wrapping_add(0x4));
                m.write32(ebx.wrapping_add(0x4), ebp);
                ebp = m.read32(ecx.wrapping_add(0x8));
                ecx = m.read32(ecx.wrapping_add(0xc));
                m.write32(ebx.wrapping_add(0x8), ebp);
                m.write32(ebx.wrapping_add(0xc), ecx);
                ecx = m.read32(0x7844a8);
                ebx = (((((m.read16(ecx.wrapping_add(0x2f12)) as i16)) as i32)) as u32);
                edx = edx.wrapping_sub(1);
                eax = eax.wrapping_sub(0x10);
                fa = edx;
                fb = ebx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x3052f2; continue; }
                pc = 0x305322; continue;
            }
            0x305322 => {
                edx = (((((m.read16(ecx.wrapping_add(0x2f12)) as i16)) as i32)) as u32);
                eax = m.read32(ecx.wrapping_add(0x2f18));
                edx = edx.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                m.write32(edx.wrapping_add(esi).wrapping_add(0x64), eax);
                eax = m.read32(0x7844a8);
                ecx = (((((m.read16(eax.wrapping_add(0x2f12)) as i16)) as i32)) as u32);
                edx = (edx & 0xffff_0000) | ((((m.read16(eax.wrapping_add(0x2f28))) as u32)) & 0xffff);
                ecx = ecx.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                write16(m, ecx.wrapping_add(esi).wrapping_add(0x68), (((((edx) as u16)) as u32)) as u16);
                eax = m.read32(0x7844a8);
                ecx = (((((m.read16(eax.wrapping_add(0x2f12)) as i16)) as i32)) as u32);
                edx = (edx & 0xffff_0000) | ((((m.read16(eax.wrapping_add(0x2f2a))) as u32)) & 0xffff);
                ecx = ecx.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                write16(m, ecx.wrapping_add(esi).wrapping_add(0x6a), (((((edx) as u16)) as u32)) as u16);
                eax = m.read32(0x7844a8);
                ecx = (((((m.read16(eax.wrapping_add(0x2f12)) as i16)) as i32)) as u32);
                edx = (edx & 0xffff_0000) | ((((m.read16(eax.wrapping_add(0x2f2c))) as u32)) & 0xffff);
                ecx = ecx.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                write16(m, ecx.wrapping_add(esi).wrapping_add(0x6c), (((((edx) as u16)) as u32)) as u16);
                eax = m.read32(0x7844a8);
                ecx = (((((m.read16(eax.wrapping_add(0x2f12)) as i16)) as i32)) as u32);
                ecx = ecx.wrapping_add(0x7);
                ecx = ecx.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                m.write32(ecx.wrapping_add(esi), 0);
                ecx = m.read32(esi.wrapping_add(0x58));
                eax = m.read32(esi.wrapping_add(0x34));
                ecx = ecx.wrapping_add(1);
                fa = eax;
                fb = ((((0x5i32)) as u32) & 0xffffffffu32);
                m.write32(esi.wrapping_add(0x58), ecx);
                if (fa != fb) { pc = 0x3053c3; continue; }
                pc = 0x3053af; continue;
            }
            0x3053af => {
                edx = m.read32(0x7844a8);
                fa = ((((m.read8(edx.wrapping_add(0x238)))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x3053c3; continue; }
                pc = 0x3053be; continue;
            }
            0x3053be => {
                call!(0x31be40);
                pc = 0x3053c3; continue;
            }
            0x3053c3 => {
                push!(0x157a);
                call!(0x2f79e0);
                pc = 0x3053cd; continue;
            }
            0x3053cd => {
                pc = 0x3053da; continue;
            }
            0x3053cf => {
                eax = m.read32(0x7842a8);
                push!(eax);
                call!(0x2fb750);
                pc = 0x3053da; continue;
            }
            0x3053da => {
                ecx = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x4);
                write16(m, ecx.wrapping_add(0x2f10), (((((edi) as u16)) as u32)) as u16);
                call!(0x73c80);
                pc = 0x3053ef; continue;
            }
            0x3053ef => {
                eax = m.read32(0x7844a8);
                pc = 0x3053f4; continue;
            }
            0x3053f4 => {
                ecx = m.read32(esp.wrapping_add(0x14));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x305421; continue; }
                pc = 0x3053fc; continue;
            }
            0x3053fc => {
                fa = ((((m.read16(eax.wrapping_add(0x2f10)))) as u32) & 0xffffu32);
                fb = ((((0i32)) as u32) & 0xffffu32);
                fas = (((((fa)) as i16)) as i32);
                fbs = (((((fb)) as i16)) as i32);
                if (fas >= fbs) { pc = 0x305421; continue; }
                pc = 0x305406; continue;
            }
            0x305406 => {
                ecx = m.read32(eax.wrapping_add(0x2f20));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x305421; continue; }
                pc = 0x305410; continue;
            }
            0x305410 => {
                write16(m, eax.wrapping_add(0x2f10), (((((edi) as u16)) as u32)) as u16);
                call!(0x2fb020);
                pc = 0x30541c; continue;
            }
            0x30541c => {
                call!(0x73c80);
                pc = 0x305421; continue;
            }
            0x305421 => {
                edx = m.read32(0x7ba9e0);
                edx = ((edx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                eax = m.read32(edx.wrapping_add(0x7ba26c));
                push!(eax);
                call!(0x77470);
                pc = 0x305439; continue;
            }
            0x305439 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                eax = m.read32(0x7844a8);
                ebx = (ebx & 0xffff_ff00) | ((0x80) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((((ecx) as u8))) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x30549b; continue; }
                pc = 0x30544d; continue;
            }
            0x30544d => {
                ecx = (ecx & 0xffff_0000) | ((((m.read16(eax.wrapping_add(0x2f10))) as u32)) & 0xffff);
                fa = ((((((ecx) as u16))) as u32) & 0xffffu32);
                fb = ((((0i32)) as u32) & 0xffffu32);
                fas = (((((fa)) as i16)) as i32);
                fbs = (((((fb)) as i16)) as i32);
                if (fas < fbs) { pc = 0x30549b; continue; }
                pc = 0x305459; continue;
            }
            0x305459 => {
                ecx = ((((ecx) as u16)) as u16 as i16 as i32 as u32);
                edx = ecx;
                edx = ((edx as i32).wrapping_mul((0xe4) as i32)) as u32;
                eax = edx.wrapping_add(eax).wrapping_add(0x288);
                fa = m.read32(eax.wrapping_add(0x34));
                fb = ((((0x5i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x305481; continue; }
                pc = 0x305471; continue;
            }
            0x305471 => {
                edx = m.read32(eax.wrapping_add(0x58));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x305481; continue; }
                pc = 0x305478; continue;
            }
            0x305478 => {
                push!(ecx);
                call!(0x31c8b0);
                pc = 0x30547e; continue;
            }
            0x30547e => {
                esp = esp.wrapping_add(0x4);
                pc = 0x305481; continue;
            }
            0x305481 => {
                call!(0x323220);
                pc = 0x305486; continue;
            }
            0x305486 => {
                eax = m.read32(0x7844a8);
                write16(m, eax.wrapping_add(0x2f10), (((((edi) as u16)) as u32)) as u16);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                tail_to!(0x73c80); break 'run;  // direct tail jump
            }
            0x30549b => {
                ecx = (ecx & 0xffff_0000) | ((((m.read16(eax.wrapping_add(0x2f10))) as u32)) & 0xffff);
                fa = ((((((ecx) as u16))) as u32) & 0xffffu32);
                fb = ((((0i32)) as u32) & 0xffffu32);
                fas = (((((fa)) as i16)) as i32);
                fbs = (((((fb)) as i16)) as i32);
                if (fas < fbs) { pc = 0x30557a; continue; }
                pc = 0x3054ab; continue;
            }
            0x3054ab => {
                fa = ((((m.read8(eax.wrapping_add(0x25c)))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x30557a; continue; }
                pc = 0x3054b8; continue;
            }
            0x3054b8 => {
                ecx = ((((ecx) as u16)) as u16 as i16 as i32 as u32);
                ecx = ((ecx as i32).wrapping_mul((0xe4) as i32)) as u32;
                esi = ecx.wrapping_add(eax).wrapping_add(0x288);
                ecx = m.read32(esi.wrapping_add(0x34));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3054dd; continue; }
                pc = 0x3054cf; continue;
            }
            0x3054cf => {
                fa = ecx;
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3054dd; continue; }
                pc = 0x3054d4; continue;
            }
            0x3054d4 => {
                fa = ecx;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x30557a; continue; }
                pc = 0x3054dd; continue;
            }
            0x3054dd => {
                edx = m.read32(0x7ba9e0);
                edx = ((edx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                eax = m.read32(edx.wrapping_add(0x7ba26c));
                push!(eax);
                call!(0x77470);
                pc = 0x3054f5; continue;
            }
            0x3054f5 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x40i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x305529; continue; }
                pc = 0x305503; continue;
            }
            0x305503 => {
                eax = m.read32(esi.wrapping_add(0x54));
                eax = eax.wrapping_sub(1);
                fa = eax;
                fb = ((((0x7i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x305510; continue; }
                pc = 0x30550c; continue;
            }
            0x30550c => {
                eax = 0;
                pc = 0x305519; continue;
            }
            0x305510 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x305519; continue; }
                pc = 0x305514; continue;
            }
            0x305514 => {
                eax = 0x7;
                pc = 0x305519; continue;
            }
            0x305519 => {
                push!(0x1579);
                m.write32(esi.wrapping_add(0x54), eax);
                call!(0x2f79e0);
                pc = 0x305526; continue;
            }
            0x305526 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x305529; continue;
            }
            0x305529 => {
                ecx = m.read32(0x7ba9e0);
                ecx = ((ecx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                edx = m.read32(ecx.wrapping_add(0x7ba26c));
                push!(edx);
                call!(0x77470);
                pc = 0x305541; continue;
            }
            0x305541 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x305575; continue; }
                pc = 0x30554f; continue;
            }
            0x30554f => {
                eax = m.read32(esi.wrapping_add(0x54));
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ((((0x7i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x30555c; continue; }
                pc = 0x305558; continue;
            }
            0x305558 => {
                eax = 0;
                pc = 0x305565; continue;
            }
            0x30555c => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x305565; continue; }
                pc = 0x305560; continue;
            }
            0x305560 => {
                eax = 0x7;
                pc = 0x305565; continue;
            }
            0x305565 => {
                push!(0x1579);
                m.write32(esi.wrapping_add(0x54), eax);
                call!(0x2f79e0);
                pc = 0x305572; continue;
            }
            0x305572 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x305575; continue;
            }
            0x305575 => {
                eax = m.read32(0x7844a8);
                pc = 0x30557a; continue;
            }
            0x30557a => {
                ecx = m.read32(eax.wrapping_add(0x2f20));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x30586e; continue; }
                pc = 0x305588; continue;
            }
            0x305588 => {
                fa = ((((m.read16(eax.wrapping_add(0x2f10)))) as u32) & 0xffffu32);
                fb = ((((0i32)) as u32) & 0xffffu32);
                fas = (((((fa)) as i16)) as i32);
                fbs = (((((fb)) as i16)) as i32);
                if (fas >= fbs) { pc = 0x30586e; continue; }
                pc = 0x305596; continue;
            }
            0x305596 => {
                edx = m.read32(0x7ba9e0);
                edi = ecx;
                edx = ((edx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                ecx = (ecx & 0xffff);
                ebp = ecx;
                edi = (((((edi as i32)).wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                ecx = edi;
                ecx = ((ecx as i32).wrapping_mul((0xe4) as i32)) as u32;
                esi = ecx.wrapping_add(eax).wrapping_add(0x288);
                eax = m.read32(edx.wrapping_add(0x7ba26c));
                push!(eax);
                m.write32(esp.wrapping_add(0x18), ebp);
                call!(0x77470);
                pc = 0x3055ce; continue;
            }
            0x3055ce => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((((ecx) as u8))) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x305619; continue; }
                pc = 0x3055db; continue;
            }
            0x3055db => {
                ecx = m.read32(0x7ba9e0);
                ecx = ((ecx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                edx = m.read32(ecx.wrapping_add(0x7ba26c));
                push!(edx);
                call!(0x77470);
                pc = 0x3055f3; continue;
            }
            0x3055f3 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x30576d; continue; }
                pc = 0x305605; continue;
            }
            0x305605 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x25c));
                fa = (((((((ecx) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x30576d; continue; }
                pc = 0x305619; continue;
            }
            0x305619 => {
                call!(0x323220);
                pc = 0x30561e; continue;
            }
            0x30561e => {
                ecx = m.read32(0x7ba9e0);
                ecx = ((ecx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                edx = m.read32(ecx.wrapping_add(0x7ba26c));
                push!(edx);
                call!(0x77470);
                pc = 0x305636; continue;
            }
            0x305636 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x30568f; continue; }
                pc = 0x305644; continue;
            }
            0x305644 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x25c));
                fa = (((((((ecx) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x30568f; continue; }
                pc = 0x305654; continue;
            }
            0x305654 => {
                push!(0x1578);
                call!(0x2f79e0);
                pc = 0x30565e; continue;
            }
            0x30565e => {
                ecx = m.read32(0x7844a8);
                eax = (eax & 0xffff_ff00) | ((((m.read8(ecx.wrapping_add(0x238))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x30568f; continue; }
                pc = 0x305671; continue;
            }
            0x305671 => {
                push!(0xf);
                call!(0x3370a0);
                pc = 0x305678; continue;
            }
            0x305678 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x238));
                esp = esp.wrapping_add(0x4);
                ecx = (ecx | 0x20);
                m.write32(eax.wrapping_add(0x238), ecx);
                pc = 0x30568f; continue;
            }
            0x30568f => {
                edx = m.read32(0x7ba9e0);
                edx = ((edx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                eax = m.read32(edx.wrapping_add(0x7ba26c));
                push!(eax);
                call!(0x77470);
                pc = 0x3056a7; continue;
            }
            0x3056a7 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((((ecx) as u8))) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x3056ef; continue; }
                pc = 0x3056b4; continue;
            }
            0x3056b4 => {
                push!(0x1577);
                call!(0x2f79e0);
                pc = 0x3056be; continue;
            }
            0x3056be => {
                ecx = m.read32(0x7844a8);
                eax = (eax & 0xffff_ff00) | ((((m.read8(ecx.wrapping_add(0x238))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x3056ef; continue; }
                pc = 0x3056d1; continue;
            }
            0x3056d1 => {
                push!(0xe);
                call!(0x3370a0);
                pc = 0x3056d8; continue;
            }
            0x3056d8 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x238));
                esp = esp.wrapping_add(0x4);
                ecx = (ecx | 0x20);
                m.write32(eax.wrapping_add(0x238), ecx);
                pc = 0x3056ef; continue;
            }
            0x3056ef => {
                edx = m.read32(esi.wrapping_add(0x58));
                edx = edx.wrapping_sub(1);
                fa = ebp;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x30572d; continue; }
                pc = 0x3056f7; continue;
            }
            0x3056f7 => {
                ecx = ebp;
                ebp = ebp.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = esi.wrapping_add(ebp).wrapping_add(0x64);
                pc = 0x305700; continue;
            }
            0x305700 => {
                edx = eax.wrapping_add(0x10);
                ebp = m.read32(edx);
                ebx = eax;
                m.write32(ebx, ebp);
                ebp = m.read32(edx.wrapping_add(0x4));
                m.write32(ebx.wrapping_add(0x4), ebp);
                ebp = m.read32(edx.wrapping_add(0x8));
                edx = m.read32(edx.wrapping_add(0xc));
                m.write32(ebx.wrapping_add(0x8), ebp);
                m.write32(ebx.wrapping_add(0xc), edx);
                edx = m.read32(esi.wrapping_add(0x58));
                ecx = ecx.wrapping_add(1);
                eax = eax.wrapping_add(0x10);
                edx = edx.wrapping_sub(1);
                fa = ecx;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x305700; continue; }
                pc = 0x305727; continue;
            }
            0x305727 => {
                ebp = m.read32(esp.wrapping_add(0x14));
                ebx = (ebx & 0xffff_ff00) | ((0x80) & 0xff);
                pc = 0x30572d; continue;
            }
            0x30572d => {
                ecx = m.read32(esi.wrapping_add(0x58));
                ecx = ecx.wrapping_sub(1);
                eax = ecx;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esi.wrapping_add(0x58), ecx);
                if (fas > fbs) { pc = 0x30576d; continue; }
                pc = 0x30573a; continue;
            }
            0x30573a => {
                fa = m.read32(esi.wrapping_add(0x34));
                fb = ((((0x5i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x30576d; continue; }
                pc = 0x305740; continue;
            }
            0x305740 => {
                eax = m.read32(0x7ba9e0);
                eax = ((eax as i32).wrapping_mul((0x1e0) as i32)) as u32;
                ecx = m.read32(eax.wrapping_add(0x7ba26c));
                push!(ecx);
                call!(0x77470);
                pc = 0x305757; continue;
            }
            0x305757 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((((ecx) as u8))) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x30576d; continue; }
                pc = 0x305764; continue;
            }
            0x305764 => {
                push!(edi);
                call!(0x31c8b0);
                pc = 0x30576a; continue;
            }
            0x30576a => {
                esp = esp.wrapping_add(0x4);
                pc = 0x30576d; continue;
            }
            0x30576d => {
                edx = m.read32(0x7ba9e0);
                edx = ((edx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                eax = m.read32(edx.wrapping_add(0x7ba26c));
                push!(eax);
                call!(0x77470);
                pc = 0x305785; continue;
            }
            0x305785 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x3057bc; continue; }
                pc = 0x305793; continue;
            }
            0x305793 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x25c));
                fa = (((((((ecx) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x3057bc; continue; }
                pc = 0x3057a3; continue;
            }
            0x3057a3 => {
                write16(m, eax.wrapping_add(0x2f10), (((((edi) as u16)) as u32)) as u16);
                ecx = m.read32(0x7844a8);
                write16(m, ecx.wrapping_add(0x2f12), (((((ebp) as u16)) as u32)) as u16);
                call!(0x73c80);
                pc = 0x3057bc; continue;
            }
            0x3057bc => {
                edx = m.read32(0x7ba9e0);
                edx = ((edx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                eax = m.read32(edx.wrapping_add(0x7ba26c));
                push!(eax);
                call!(0x77470);
                pc = 0x3057d4; continue;
            }
            0x3057d4 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x305816; continue; }
                pc = 0x3057e2; continue;
            }
            0x3057e2 => {
                push!(edi);
                call!(0x31ba90);
                pc = 0x3057e8; continue;
            }
            0x3057e8 => {
                push!(esi);
                call!(0x31d2d0);
                pc = 0x3057ee; continue;
            }
            0x3057ee => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x305809; continue; }
                pc = 0x3057f5; continue;
            }
            0x3057f5 => {
                push!(esi);
                call!(0x31d2c0);
                pc = 0x3057fb; continue;
            }
            0x3057fb => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x305809; continue; }
                pc = 0x305802; continue;
            }
            0x305802 => {
                push!(0x52e0a8);
                pc = 0x30580e; continue;
            }
            0x305809 => {
                push!(0x52e098);
                pc = 0x30580e; continue;
            }
            0x30580e => {
                call!(0x2fb230);
                pc = 0x305813; continue;
            }
            0x305813 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x305816; continue;
            }
            0x305816 => {
                ecx = m.read32(0x7ba9e0);
                ecx = ((ecx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                edx = m.read32(ecx.wrapping_add(0x7ba26c));
                push!(edx);
                call!(0x77470);
                pc = 0x30582e; continue;
            }
            0x30582e => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x40i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x30586e; continue; }
                pc = 0x30583c; continue;
            }
            0x30583c => {
                fa = m.read32(esi.wrapping_add(0x58));
                fb = ((((0x8i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x30586e; continue; }
                pc = 0x305842; continue;
            }
            0x305842 => {
                push!(esi);
                call!(0x31d2d0);
                pc = 0x305848; continue;
            }
            0x305848 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30586e; continue; }
                pc = 0x30584f; continue;
            }
            0x30584f => {
                eax = m.read32(0x7844a8);
                write16(m, eax.wrapping_add(0x2f10), (((((edi) as u16)) as u32)) as u16);
                ecx = m.read32(0x7844a8);
                ebp = ebp.wrapping_add(1);
                write16(m, ecx.wrapping_add(0x2f12), (((((ebp) as u16)) as u32)) as u16);
                call!(0x73c80);
                pc = 0x30586e; continue;
            }
            0x30586e => {
                edx = m.read32(0x7ba9e0);
                edx = ((edx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                eax = m.read32(edx.wrapping_add(0x7ba26c));
                push!(eax);
                call!(0x77470);
                pc = 0x305886; continue;
            }
            0x305886 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x3058a3; continue; }
                pc = 0x305894; continue;
            }
            0x305894 => {
                eax = m.read32(0x7844a8);
                m.write32(eax.wrapping_add(0x25c), (m.read32(eax.wrapping_add(0x25c)) | 0x2000));
                pc = 0x3058a3; continue;
            }
            0x3058a3 => {
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
/// 0x003058B0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3058b0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3058b0;
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
        { crate::pose::cover(0x3058b0, previous, pc); previous = pc; }
        match pc {
            0x3058b0 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x11858));
                edx = (((((m.read16(ecx.wrapping_add(0xc)) as i16)) as i32)) as u32);
                edx = ((edx as i32).wrapping_mul((0x8c) as i32)) as u32;
                push!(ebx);
                push!(esi);
                esi = edx.wrapping_add(eax).wrapping_add(0xdd14);
                eax = m.read32(0x784014);
                fa = m.read32(eax);
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                push!(edi);
                if (fa == fb) { pc = 0x3058e7; continue; }
                pc = 0x3058d9; continue;
            }
            0x3058d9 => {
                push!(0xc77668);
                push!(eax);
                call!(0x32a5b0);
                pc = 0x3058e4; continue;
            }
            0x3058e4 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x3058e7; continue;
            }
            0x3058e7 => {
                edi = m.read32(esi.wrapping_add(0x20));
                push!(0x3);
                edi = edi.wrapping_add(1);
                call!(0x321290);
                pc = 0x3058f2; continue;
            }
            0x3058f2 => {
                ebx = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                ebx = (ebx & 0xffff_ff00) | ((((if ((fas > fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                push!(0x1);
                eax = esi;
                ebx = ebx.wrapping_sub(1);
                ebx = (ebx & 0x8);
                call!(0x2fba30);
                pc = 0x305906; continue;
            }
            0x305906 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3059d0; continue; }
                pc = 0x305911; continue;
            }
            0x305911 => {
                eax = ebx.wrapping_add(edi.wrapping_mul(0x8));
                push!(eax);
                call!(0x300670);
                pc = 0x30591a; continue;
            }
            0x30591a => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x305934; continue; }
                pc = 0x305921; continue;
            }
            0x305921 => {
                ecx = m.read32(0x7842a8);
                push!(ecx);
                call!(0x2fb750);
                pc = 0x30592d; continue;
            }
            0x30592d => {
                esp = esp.wrapping_add(0x4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x305934 => {
                edi = m.read32(esi.wrapping_add(0x20));
                fa = edi;
                fb = ((((0xai32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x305993; continue; }
                pc = 0x30593c; continue;
            }
            0x30593c => {
                push!(edi);
                push!(esi);
                call!(0x320fa0);
                pc = 0x305943; continue;
            }
            0x305943 => {
                edx = m.read32(0x7844a8);
                ecx = m.read32(edx.wrapping_add(0x2f18));
                m.write32(esi.wrapping_add(edi.wrapping_mul(0x8)).wrapping_add(0x24), ecx);
                edx = m.read32(0x7844a8);
                ecx = (ecx & 0xffff_0000) | ((((m.read16(edx.wrapping_add(0x2f28))) as u32)) & 0xffff);
                write16(m, eax, (((((ecx) as u16)) as u32)) as u16);
                edx = m.read32(0x7844a8);
                ecx = (ecx & 0xffff_0000) | ((((m.read16(edx.wrapping_add(0x2f2a))) as u32)) & 0xffff);
                write16(m, eax.wrapping_add(0x2), (((((ecx) as u16)) as u32)) as u16);
                edx = m.read32(0x7844a8);
                ecx = (ecx & 0xffff_0000) | ((((m.read16(edx.wrapping_add(0x2f2c))) as u32)) & 0xffff);
                write16(m, eax.wrapping_add(0x4), (((((ecx) as u16)) as u32)) as u16);
                eax = m.read32(esi.wrapping_add(0x20));
                esp = esp.wrapping_add(0x8);
                eax = eax.wrapping_add(1);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(esi.wrapping_add(0x20), eax);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x305993 => {
                push!(0xa);
                push!(0xbc0);
                call!(0x3d7b0);
                pc = 0x30599f; continue;
            }
            0x30599f => {
                edx = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(edx);
                call!(0x3c85d4);
                pc = 0x3059af; continue;
            }
            0x3059af => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(0x7842a8);
                push!(0);
                push!(eax);
                push!(ecx);
                call!(0x7a270);
                pc = 0x3059c3; continue;
            }
            0x3059c3 => {
                push!(0x1575);
                call!(0x2f79e0);
                pc = 0x3059cd; continue;
            }
            0x3059cd => {
                esp = esp.wrapping_add(0x1c);
                pc = 0x3059d0; continue;
            }
            0x3059d0 => {
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

/// 0x003063C0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3063c0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3063c0;
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
        { crate::pose::cover(0x3063c0, previous, pc); previous = pc; }
        match pc {
            0x3063c0 => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x254));
                fa = eax;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                if (fa > fb) { pc = 0x3063e1; continue; }
                pc = 0x3063d1; continue;
            }
            0x3063d1 => {
                jt = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x306444));
                if (jt == 0x3063d8u32) { pc = 0x3063d8; continue; }
                if (jt == 0x3063e2u32) { pc = 0x3063e2; continue; }
                if (jt == 0x30642cu32) { pc = 0x30642c; continue; }
                if (jt == 0x306433u32) { pc = 0x306433; continue; }
                if (jt == 0x30643du32) { pc = 0x30643d; continue; }
                tail_to!(jt); break 'run;  // indirect tail jump, target outside the switch
            }
            0x3063d8 => {
                push!(esi);
                call!(0x303e10);
                pc = 0x3063de; continue;
            }
            0x3063de => {
                esp = esp.wrapping_add(0x4);
                pc = 0x3063e1; continue;
            }
            0x3063e1 => {
                break 'run;
            }
            0x3063e2 => {
                fa = m.read32(ecx.wrapping_add(0x1185c));
                fb = ((((0x8i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30643d; continue; }
                pc = 0x3063eb; continue;
            }
            0x3063eb => {
                call!(0x304810);
                pc = 0x3063f0; continue;
            }
            0x3063f0 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3063e1; continue; }
                pc = 0x3063f4; continue;
            }
            0x3063f4 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f34));
                ecx = ((ecx as i32).wrapping_mul((0xc4) as i32)) as u32;
                fa = ((((m.read16(ecx.wrapping_add(0x524f5a)))) as u32) & 0xffffu32);
                fb = ((((0x22i32)) as u32) & 0xffffu32);
                if (fa == fb) { pc = 0x3063e1; continue; }
                pc = 0x30640f; continue;
            }
            0x30640f => {
                ecx = m.read32(eax.wrapping_add(0x2f1c));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x306422; continue; }
                pc = 0x306419; continue;
            }
            0x306419 => {
                fa = m.read32(eax.wrapping_add(0x254));
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3063e1; continue; }
                pc = 0x306422; continue;
            }
            0x306422 => {
                push!(esi);
                call!(0x304c30);
                pc = 0x306428; continue;
            }
            0x306428 => {
                esp = esp.wrapping_add(0x4);
                break 'run;
            }
            0x30642c => {
                eax = esi;
                tail_to!(0x3000a0); break 'run;  // direct tail jump
            }
            0x306433 => {
                push!(esi);
                call!(0x3051f0);
                pc = 0x306439; continue;
            }
            0x306439 => {
                esp = esp.wrapping_add(0x4);
                break 'run;
            }
            0x30643d => {
                tail_to!(0x3059e0); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00307CF0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f307cf0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x307cf0;
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
        { crate::pose::cover(0x307cf0, previous, pc); previous = pc; }
        match pc {
            0x307cf0 => {
                ecx = m.read32(esp.wrapping_add(0x4));
                eax = m.read32(ecx.wrapping_add(0xc));
                push!(ebx);
                push!(esi);
                esi = m.read32(ecx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x2c));
                eax = m.read32(esi.wrapping_add(0x1c));
                ebx = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x307d96; continue; }
                pc = 0x307d0a; continue;
            }
            0x307d0a => {
                xmm2 = Xmm::scalar(m.read32(0x47d640));
                push!(ebp);
                push!(edi);
                pc = 0x307d14; continue;
            }
            0x307d14 => {
                eax = m.read32(esi.wrapping_add(0x18));
                edi = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x307d8c; continue; }
                pc = 0x307d1d; continue;
            }
            0x307d1d => {
                pc = 0x307d20; continue;
            }
            0x307d20 => {
                eax = m.read32(esi.wrapping_add(0x14));
                edx = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x307d84; continue; }
                pc = 0x307d29; continue;
            }
            0x307d29 => {
                xmm0.lanes[0] = (((ebx) as i32) as f32).to_bits();
                xmm1.lanes[0] = (((edi) as i32) as f32).to_bits();
                pc = 0x307d31; continue;
            }
            0x307d31 => {
                xmm3 = xmm0;
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x8)))).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                eax = sse::cvttss2si(f32::from_bits(xmm3.lanes[0]));
                eax = eax.wrapping_add(eax.wrapping_mul(0x4));
                xmm3 = xmm1;
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x4)))).to_bits();
                ebp = sse::cvttss2si(f32::from_bits(xmm3.lanes[0]));
                eax = ebp.wrapping_add(eax.wrapping_mul(0x8));
                xmm3.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(ecx))).to_bits();
                eax = eax.wrapping_add(eax.wrapping_mul(0x4));
                ebp = sse::cvttss2si(f32::from_bits(xmm3.lanes[0]));
                eax = ebp.wrapping_add(eax.wrapping_mul(0x8));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x307d7c; continue; }
                pc = 0x307d6b; continue;
            }
            0x307d6b => {
                fa = eax;
                fb = ((((0x2580i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x307d7c; continue; }
                pc = 0x307d72; continue;
            }
            0x307d72 => {
                ebp = m.read32(0x784004);
                m.write32(ebp.wrapping_add(eax.wrapping_mul(0x4)), ecx);
                pc = 0x307d7c; continue;
            }
            0x307d7c => {
                eax = m.read32(esi.wrapping_add(0x14));
                edx = edx.wrapping_add(1);
                fa = edx;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x307d31; continue; }
                pc = 0x307d84; continue;
            }
            0x307d84 => {
                eax = m.read32(esi.wrapping_add(0x18));
                edi = edi.wrapping_add(1);
                fa = edi;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x307d20; continue; }
                pc = 0x307d8c; continue;
            }
            0x307d8c => {
                eax = m.read32(esi.wrapping_add(0x1c));
                ebx = ebx.wrapping_add(1);
                fa = ebx;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x307d14; continue; }
                pc = 0x307d94; continue;
            }
            0x307d94 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x307d96; continue;
            }
            0x307d96 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00307DA0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f307da0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x307da0;
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
        { crate::pose::cover(0x307da0, previous, pc); previous = pc; }
        match pc {
            0x307da0 => {
                ecx = m.read32(esp.wrapping_add(0x4));
                eax = m.read32(ecx.wrapping_add(0xc));
                push!(ebx);
                push!(esi);
                esi = m.read32(ecx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x2c));
                eax = m.read32(esi.wrapping_add(0x1c));
                ebx = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x307e4e; continue; }
                pc = 0x307dba; continue;
            }
            0x307dba => {
                xmm2 = Xmm::scalar(m.read32(0x47d640));
                push!(ebp);
                push!(edi);
                pc = 0x307dc4; continue;
            }
            0x307dc4 => {
                eax = m.read32(esi.wrapping_add(0x18));
                edi = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x307e40; continue; }
                pc = 0x307dcd; continue;
            }
            0x307dcd => {
                pc = 0x307dd0; continue;
            }
            0x307dd0 => {
                eax = m.read32(esi.wrapping_add(0x14));
                edx = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x307e38; continue; }
                pc = 0x307dd9; continue;
            }
            0x307dd9 => {
                xmm0.lanes[0] = (((ebx) as i32) as f32).to_bits();
                xmm1.lanes[0] = (((edi) as i32) as f32).to_bits();
                pc = 0x307de1; continue;
            }
            0x307de1 => {
                xmm3 = xmm0;
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x8)))).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                eax = sse::cvttss2si(f32::from_bits(xmm3.lanes[0]));
                eax = eax.wrapping_add(eax.wrapping_mul(0x4));
                xmm3 = xmm1;
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x4)))).to_bits();
                ebp = sse::cvttss2si(f32::from_bits(xmm3.lanes[0]));
                eax = ebp.wrapping_add(eax.wrapping_mul(0x8));
                xmm3.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(ecx))).to_bits();
                eax = eax.wrapping_add(eax.wrapping_mul(0x4));
                ebp = sse::cvttss2si(f32::from_bits(xmm3.lanes[0]));
                eax = ebp.wrapping_add(eax.wrapping_mul(0x8));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x307e30; continue; }
                pc = 0x307e1b; continue;
            }
            0x307e1b => {
                fa = eax;
                fb = ((((0x2580i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x307e30; continue; }
                pc = 0x307e22; continue;
            }
            0x307e22 => {
                ebp = m.read32(0x784004);
                m.write32(ebp.wrapping_add(eax.wrapping_mul(0x4)), 0);
                pc = 0x307e30; continue;
            }
            0x307e30 => {
                eax = m.read32(esi.wrapping_add(0x14));
                edx = edx.wrapping_add(1);
                fa = edx;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x307de1; continue; }
                pc = 0x307e38; continue;
            }
            0x307e38 => {
                eax = m.read32(esi.wrapping_add(0x18));
                edi = edi.wrapping_add(1);
                fa = edi;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x307dd0; continue; }
                pc = 0x307e40; continue;
            }
            0x307e40 => {
                eax = m.read32(esi.wrapping_add(0x1c));
                ebx = ebx.wrapping_add(1);
                fa = ebx;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x307dc4; continue; }
                pc = 0x307e4c; continue;
            }
            0x307e4c => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x307e4e; continue;
            }
            0x307e4e => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00307EC0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f307ec0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x307ec0;
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
        { crate::pose::cover(0x307ec0, previous, pc); previous = pc; }
        match pc {
            0x307ec0 => {
                edx = m.read32(0x762cc4);
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x307ee5; continue; }
                pc = 0x307eca; continue;
            }
            0x307eca => {
                eax = m.read32(edx.wrapping_add(0x40));
                push!(edi);
                m.write32(0x762cc4, eax);
                eax = 0;
                ecx = 0x12;
                edi = edx;
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), eax); } edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep stosd, DF from entry.eflags
                ecx = 0;
                m.write32(0x783ff8, m.read32(0x783ff8).wrapping_sub(0x1));
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x307ee5; continue;
            }
            0x307ee5 => {
                eax = edx;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00307EF0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f307ef0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x307ef0;
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
        { crate::pose::cover(0x307ef0, previous, pc); previous = pc; }
        match pc {
            0x307ef0 => {
                push!(ebx);
                push!(esi);
                esi = m.read32(esp.wrapping_add(0xc));
                push!(edi);
                ebx = 0;
                edi = 0;
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x307f3a; continue; }
                pc = 0x307eff; continue;
            }
            0x307eff => {
                push!(ebp);
                pc = 0x307f00; continue;
            }
            0x307f00 => {
                eax = m.read32(esi.wrapping_add(0x3c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x307f11; continue; }
                pc = 0x307f07; continue;
            }
            0x307f07 => {
                eax = m.read32(eax.wrapping_add(0x14));
                edi = edi.wrapping_add(0x1c);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x307f07; continue; }
                pc = 0x307f11; continue;
            }
            0x307f11 => {
                eax = m.read32(esi.wrapping_add(0x10));
                push!(eax);
                call!(0x333700);
                pc = 0x307f1a; continue;
            }
            0x307f1a => {
                ecx = m.read32(esi.wrapping_add(0x10));
                push!(ecx);
                ebp = eax;
                call!(0x2fdf50);
                pc = 0x307f25; continue;
            }
            0x307f25 => {
                esi = m.read32(esi.wrapping_add(0x40));
                ebp = ebp.wrapping_add(eax);
                esp = esp.wrapping_add(0x8);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ebx = ebx.wrapping_add(ebp.wrapping_mul(0x8)).wrapping_add(0x28);
                if (fa != fb) { pc = 0x307f00; continue; }
                pc = 0x307f35; continue;
            }
            0x307f35 => {
                esi = m.read32(esp.wrapping_add(0x14));
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x307f3a; continue;
            }
            0x307f3a => {
                eax = m.read32(esp.wrapping_add(0x14));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x307f4a; continue; }
                pc = 0x307f42; continue;
            }
            0x307f42 => {
                ecx = m.read32(eax);
                edi = edi.wrapping_add(ebx);
                ecx = ecx.wrapping_add(edi);
                m.write32(eax, ecx);
                pc = 0x307f4a; continue;
            }
            0x307f4a => {
                edx = m.read32(0x7844a8);
                eax = m.read32(edx.wrapping_add(0x2f7c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x307f68; continue; }
                pc = 0x307f5a; continue;
            }
            0x307f5a => {
                eax = m.read32(0x784014);
                push!(eax);
                call!(0x2fe0c0);
                pc = 0x307f65; continue;
            }
            0x307f65 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x307f68; continue;
            }
            0x307f68 => {
                eax = m.read32(esi.wrapping_add(0x40));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x307f7d; continue; }
                pc = 0x307f6f; continue;
            }
            0x307f6f => {
                pc = 0x307f70; continue;
            }
            0x307f70 => {
                m.write32(0x784000, m.read32(0x784000).wrapping_add(0x1));
                eax = m.read32(eax.wrapping_add(0x40));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x307f70; continue; }
                pc = 0x307f7d; continue;
            }
            0x307f7d => {
                ecx = m.read32(0x78400c);
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x307f9e; continue; }
                pc = 0x307f87; continue;
            }
            0x307f87 => {
                eax = m.read32(ecx.wrapping_add(0x40));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x307f99; continue; }
                pc = 0x307f8e; continue;
            }
            0x307f8e => {
                edi = edi;
                pc = 0x307f90; continue;
            }
            0x307f90 => {
                ecx = eax;
                eax = m.read32(ecx.wrapping_add(0x40));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x307f90; continue; }
                pc = 0x307f99; continue;
            }
            0x307f99 => {
                m.write32(ecx.wrapping_add(0x40), esi);
                pc = 0x307fa4; continue;
            }
            0x307f9e => {
                m.write32(0x78400c, esi);
                pc = 0x307fa4; continue;
            }
            0x307fa4 => {
                push!(esi);
                call!(0x307cf0);
                pc = 0x307faa; continue;
            }
            0x307faa => {
                eax = m.read32(0x784000);
                esp = esp.wrapping_add(0x4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = eax.wrapping_add(1);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(0x784000, eax);
                eax = 0x1;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00307FD0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f307fd0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x307fd0;
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
        { crate::pose::cover(0x307fd0, previous, pc); previous = pc; }
        match pc {
            0x307fd0 => {
                push!(esi);
                esi = m.read32(0x762cc4);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x307ff6; continue; }
                pc = 0x307fdb; continue;
            }
            0x307fdb => {
                eax = m.read32(esi.wrapping_add(0x40));
                push!(edi);
                m.write32(0x762cc4, eax);
                eax = 0;
                ecx = 0x12;
                edi = esi;
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), eax); } edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep stosd, DF from entry.eflags
                ecx = 0;
                m.write32(0x783ff8, m.read32(0x783ff8).wrapping_sub(0x1));
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x307ff6; continue;
            }
            0x307ff6 => {
                eax = m.read32(esp.wrapping_add(0x8));
                ecx = m.read32(esp.wrapping_add(0x1c));
                m.write32(esi.wrapping_add(0x10), eax);
                m.write32(esi.wrapping_add(0x44), ecx);
                m.write8(esi.wrapping_add(0x14), (0) as u8);
                eax = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x783f40));
                m.write32(esi.wrapping_add(0x2c), eax);
                eax = m.read32(eax.wrapping_add(0x13c));
                m.write32(esi.wrapping_add(0x30), eax);
                eax = m.read32(eax.wrapping_add(0x13c));
                m.write32(esi.wrapping_add(0x34), eax);
                edx = m.read32(eax.wrapping_add(0x13c));
                eax = esi.wrapping_add(0x18);
                push!(0x1);
                push!(eax);
                m.write32(esi.wrapping_add(0x38), edx);
                call!(0x319480);
                pc = 0x308038; continue;
            }
            0x308038 => {
                ecx = (((((f32::from_bits(m.read32(esp.wrapping_add(0x14))) as f64)) as i32)) as u32);
                edx = (((((f32::from_bits(m.read32(esp.wrapping_add(0x18))) as f64)) as i32)) as u32);
                eax = m.read32(esp.wrapping_add(0x20));
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                m.write32(esi, xmm0.lanes[0]);
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                m.write32(esi.wrapping_add(0x4), xmm0.lanes[0]);
                xmm0.lanes[0] = (((m.read32(esp.wrapping_add(0x1c))) as i32) as f32).to_bits();
                m.write32(esi.wrapping_add(0xc), eax);
                esp = esp.wrapping_add(0x8);
                m.write32(esi.wrapping_add(0x8), xmm0.lanes[0]);
                eax = esi;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00308070: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f308070<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x308070;
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
        { crate::pose::cover(0x308070, previous, pc); previous = pc; }
        match pc {
            0x308070 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                edx = eax;
                m.write32(esi.wrapping_add(0x14), 0);
                if (fa != fb) { pc = 0x308080; continue; }
                pc = 0x30807d; continue;
            }
            0x30807d => {
                eax = esi;
                break 'run;
            }
            0x308080 => {
                ecx = eax.wrapping_add(0x14);
                push!(edi);
                fa = m.read32(ecx);
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30809c; continue; }
                pc = 0x308089; continue;
            }
            0x308089 => {
                pc = 0x308090; continue;
            }
            0x308090 => {
                edx = m.read32(ecx);
                edi = m.read32(edx.wrapping_add(0x14));
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ecx = edx.wrapping_add(0x14);
                if (fa != fb) { pc = 0x308090; continue; }
                pc = 0x30809c; continue;
            }
            0x30809c => {
                m.write32(edx.wrapping_add(0x14), esi);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x003080B0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3080b0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3080b0;
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
        { crate::pose::cover(0x3080b0, previous, pc); previous = pc; }
        match pc {
            0x3080b0 => {
                edx = m.read32(esp.wrapping_add(0x8));
                push!(esi);
                push!(edi);
                edi = m.read32(0x762cc8);
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = edi;
                if (fa == fb) { pc = 0x3080cd; continue; }
                pc = 0x3080c2; continue;
            }
            0x3080c2 => {
                fa = eax;
                fb = edx;
                if (fa == fb) { pc = 0x3080c9; continue; }
                pc = 0x3080c6; continue;
            }
            0x3080c6 => {
                eax = m.read32(eax.wrapping_add(0x14));
                pc = 0x3080c9; continue;
            }
            0x3080c9 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3080c2; continue; }
                pc = 0x3080cd; continue;
            }
            0x3080cd => {
                esi = m.read32(esp.wrapping_add(0xc));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x308103; continue; }
                pc = 0x3080d5; continue;
            }
            0x3080d5 => {
                eax = m.read32(esi.wrapping_add(0x3c));
                ecx = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3080ed; continue; }
                pc = 0x3080de; continue;
            }
            0x3080de => {
                edi = edi;
                pc = 0x3080e0; continue;
            }
            0x3080e0 => {
                fa = eax;
                fb = edx;
                if (fa == fb) { pc = 0x3080f1; continue; }
                pc = 0x3080e4; continue;
            }
            0x3080e4 => {
                ecx = eax;
                eax = m.read32(eax.wrapping_add(0x14));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3080e0; continue; }
                pc = 0x3080ed; continue;
            }
            0x3080ed => {
                fa = eax;
                fb = edx;
                if (fa != fb) { pc = 0x308103; continue; }
                pc = 0x3080f1; continue;
            }
            0x3080f1 => {
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3080fd; continue; }
                pc = 0x3080f5; continue;
            }
            0x3080f5 => {
                eax = m.read32(edx.wrapping_add(0x14));
                m.write32(ecx.wrapping_add(0x14), eax);
                pc = 0x308103; continue;
            }
            0x3080fd => {
                ecx = m.read32(edx.wrapping_add(0x14));
                m.write32(esi.wrapping_add(0x3c), ecx);
                pc = 0x308103; continue;
            }
            0x308103 => {
                eax = m.read32(0x762cc0);
                m.write32(edx.wrapping_add(0x14), edi);
                eax = eax.wrapping_add(1);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(0x762cc8, edx);
                m.write32(0x762cc0, eax);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00308120: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f308120<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x308120;
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
        { crate::pose::cover(0x308120, previous, pc); previous = pc; }
        match pc {
            0x308120 => {
                eax = m.read32(esp.wrapping_add(0x4));
                push!(ebp);
                ebp = m.read32(eax.wrapping_add(0x3c));
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3081da; continue; }
                pc = 0x308130; continue;
            }
            0x308130 => {
                push!(ebx);
                ebx = m.read32(0x762cc8);
                push!(esi);
                push!(edi);
                pc = 0x308140; continue;
            }
            0x308140 => {
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3081d1; continue; }
                pc = 0x308148; continue;
            }
            0x308148 => {
                esi = m.read32(0x762cc0);
                ecx = (ecx | 0xffffffffu32);
                eax = ebx;
                esi = esi.wrapping_sub(1);
                write16(m, eax.wrapping_add(0xc), (((((ecx) as u16)) as u32)) as u16);
                write16(m, eax.wrapping_add(0xe), (((((ecx) as u16)) as u32)) as u16);
                write16(m, eax.wrapping_add(0x10), (((((ecx) as u16)) as u32)) as u16);
                m.write32(eax, ecx);
                m.write32(eax.wrapping_add(0x4), ecx);
                edx = ebx.wrapping_add(0x14);
                ebx = m.read32(edx);
                m.write32(0x762cc0, esi);
                ecx = 0xf;
                esi = ebp;
                edi = eax;
                { let step = if entry.eflags & 0x400 == 0 { 4u32 } else { 0u32.wrapping_sub(4) }; for i in 0..ecx { let value=m.read32(esi.wrapping_add(i.wrapping_mul(step)));m.write32(edi.wrapping_add(i.wrapping_mul(step)),value); } esi=esi.wrapping_add(ecx.wrapping_mul(step));edi=edi.wrapping_add(ecx.wrapping_mul(step));ecx=0; }
                ecx = m.read32(eax.wrapping_add(0x4));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x308189; continue; }
                pc = 0x308182; continue;
            }
            0x308182 => {
                m.write32(eax.wrapping_add(0x4), 0xffffffffu32);
                pc = 0x308189; continue;
            }
            0x308189 => {
                ecx = m.read32(esp.wrapping_add(0x18));
                esi = m.read32(ecx.wrapping_add(0x3c));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                edi = esi;
                m.write32(edx, 0);
                if (fa != fb) { pc = 0x3081a0; continue; }
                pc = 0x30819c; continue;
            }
            0x30819c => {
                esi = eax;
                pc = 0x3081bf; continue;
            }
            0x3081a0 => {
                edx = m.read32(esi.wrapping_add(0x14));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ecx = esi.wrapping_add(0x14);
                if (fa == fb) { pc = 0x3081bc; continue; }
                pc = 0x3081aa; continue;
            }
            0x3081aa => {
                pc = 0x3081b0; continue;
            }
            0x3081b0 => {
                edi = m.read32(ecx);
                edx = m.read32(edi.wrapping_add(0x14));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ecx = edi.wrapping_add(0x14);
                if (fa != fb) { pc = 0x3081b0; continue; }
                pc = 0x3081bc; continue;
            }
            0x3081bc => {
                m.write32(edi.wrapping_add(0x14), eax);
                pc = 0x3081bf; continue;
            }
            0x3081bf => {
                edx = m.read32(esp.wrapping_add(0x18));
                m.write32(edx.wrapping_add(0x3c), esi);
                ebp = m.read32(ebp.wrapping_add(0x14));
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x308140; continue; }
                pc = 0x3081d1; continue;
            }
            0x3081d1 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(0x762cc8, ebx);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x3081da; continue;
            }
            0x3081da => {
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x003081E0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3081e0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3081e0;
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
        { crate::pose::cover(0x3081e0, previous, pc); previous = pc; }
        match pc {
            0x3081e0 => {
                push!(ebx);
                ebx = m.read32(0x762cc8);
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x308215; continue; }
                pc = 0x3081eb; continue;
            }
            0x3081eb => {
                eax = m.read32(ebx.wrapping_add(0x14));
                m.write32(0x762cc8, eax);
                eax = (eax | 0xffffffffu32);
                write16(m, ebx.wrapping_add(0xc), (((((eax) as u16)) as u32)) as u16);
                write16(m, ebx.wrapping_add(0xe), (((((eax) as u16)) as u32)) as u16);
                write16(m, ebx.wrapping_add(0x10), (((((eax) as u16)) as u32)) as u16);
                m.write32(ebx, eax);
                m.write32(ebx.wrapping_add(0x4), eax);
                m.write32(0x762cc0, m.read32(0x762cc0).wrapping_sub(0x1));
                eax = m.read32(esp.wrapping_add(0x14));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x308219; continue; }
                pc = 0x308215; continue;
            }
            0x308215 => {
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x308219 => {
                edx = (edx & 0xffff_0000) | ((((m.read16(esp.wrapping_add(0x10))) as u32)) & 0xffff);
                push!(esi);
                push!(edi);
                esi = eax;
                ecx = 0xf;
                edi = ebx;
                { let step = if entry.eflags & 0x400 == 0 { 4u32 } else { 0u32.wrapping_sub(4) }; for i in 0..ecx { let value=m.read32(esi.wrapping_add(i.wrapping_mul(step)));m.write32(edi.wrapping_add(i.wrapping_mul(step)),value); } esi=esi.wrapping_add(ecx.wrapping_mul(step));edi=edi.wrapping_add(ecx.wrapping_mul(step));ecx=0; }
                ecx = m.read32(esp.wrapping_add(0x14));
                edi = m.read32(esp.wrapping_add(0x10));
                m.write32(ebx.wrapping_add(0xc), ecx);
                write16(m, ebx.wrapping_add(0x10), (((((edx) as u16)) as u32)) as u16);
                eax = m.read32(eax.wrapping_add(0x8));
                ecx = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                ecx = (ecx & 0xffff_ff00) | ((((if ((fas < fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                esi = ebx;
                ecx = ecx.wrapping_sub(1);
                ecx = (ecx & eax);
                m.write32(ebx.wrapping_add(0x8), ecx);
                eax = m.read32(edi.wrapping_add(0x3c));
                call!(0x308070);
                pc = 0x308254; continue;
            }
            0x308254 => {
                m.write32(edi.wrapping_add(0x3c), eax);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = ebx;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00308260: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f308260<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x308260;
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
        { crate::pose::cover(0x308260, previous, pc); previous = pc; }
        match pc {
            0x308260 => {
                eax = m.read32(esp.wrapping_add(0x4));
                eax = m.read32(eax.wrapping_add(0x3c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                push!(esi);
                push!(edi);
                if (fa == fb) { pc = 0x30829d; continue; }
                pc = 0x30826d; continue;
            }
            0x30826d => {
                ecx = m.read32(esp.wrapping_add(0x18));
                edx = (edx & 0xffff_0000) | ((((m.read16(esp.wrapping_add(0x14))) as u32)) & 0xffff);
                esi = m.read32(esp.wrapping_add(0x12));
                edi = (edi & 0xffff_0000) | ((((m.read16(esp.wrapping_add(0x10))) as u32)) & 0xffff);
                pc = 0x308280; continue;
            }
            0x308280 => {
                fa = ((((m.read16(eax.wrapping_add(0xc)))) as u32) & 0xffffu32);
                fb = ((((((edi) as u16))) as u32) & 0xffffu32);
                if (fa != fb) { pc = 0x308292; continue; }
                pc = 0x308286; continue;
            }
            0x308286 => {
                fa = ((((m.read16(eax.wrapping_add(0xe)))) as u32) & 0xffffu32);
                fb = ((((((esi) as u16))) as u32) & 0xffffu32);
                if (fa != fb) { pc = 0x308292; continue; }
                pc = 0x30828c; continue;
            }
            0x30828c => {
                fa = ((((m.read16(eax.wrapping_add(0x10)))) as u32) & 0xffffu32);
                fb = ((((((edx) as u16))) as u32) & 0xffffu32);
                if (fa == fb) { pc = 0x30829f; continue; }
                pc = 0x308292; continue;
            }
            0x308292 => {
                fa = m.read32(eax);
                fb = ecx;
                if (fa == fb) { pc = 0x30829f; continue; }
                pc = 0x308296; continue;
            }
            0x308296 => {
                eax = m.read32(eax.wrapping_add(0x14));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x308280; continue; }
                pc = 0x30829d; continue;
            }
            0x30829d => {
                eax = 0;
                pc = 0x30829f; continue;
            }
            0x30829f => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00308340: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f308340<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x308340;
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
        { crate::pose::cover(0x308340, previous, pc); previous = pc; }
        match pc {
            0x308340 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                push!(esi);
                call!(0x2f7da0);
                pc = 0x30834b; continue;
            }
            0x30834b => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x308379; continue; }
                pc = 0x308352; continue;
            }
            0x308352 => {
                eax = 0;
                esi = esi.wrapping_sub(0x46);
                fa = esi;
                if ((fa == 0)) { pc = 0x30836a; continue; }
                pc = 0x308359; continue;
            }
            0x308359 => {
                esi = esi.wrapping_sub(1);
                fa = esi;
                if ((fa != 0)) { pc = 0x308368; continue; }
                pc = 0x30835c; continue;
            }
            0x30835c => {
                eax = m.read32(0x7844a8);
                eax = ((m.read16(eax.wrapping_add(0x306a))) as u16 as u32);
                pc = 0x308368; continue;
            }
            0x308368 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x30836a => {
                ecx = m.read32(0x7844a8);
                eax = ((m.read16(ecx.wrapping_add(0x3068))) as u16 as u32);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x308379 => {
                fa = esi;
                fb = ((((0x9i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3083ac; continue; }
                pc = 0x30837e; continue;
            }
            0x30837e => {
                fa = esi;
                fb = ((((0x34i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x3083a1; continue; }
                pc = 0x308383; continue;
            }
            0x308383 => {
                fa = esi;
                fb = ((((0x36i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x3083a1; continue; }
                pc = 0x308388; continue;
            }
            0x308388 => {
                push!(0x36);
                call!(0x308300);
                pc = 0x30838f; continue;
            }
            0x30838f => {
                push!(0x35);
                esi = eax;
                call!(0x308300);
                pc = 0x308398; continue;
            }
            0x308398 => {
                esp = esp.wrapping_add(0x8);
                esi = esi.wrapping_add(eax);
                eax = esi;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x3083a1 => {
                push!(esi);
                call!(0x308300);
                pc = 0x3083a7; continue;
            }
            0x3083a7 => {
                esp = esp.wrapping_add(0x4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x3083ac => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                tail_to!(0x31bdb0); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x003083C0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3083c0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3083c0;
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
        { crate::pose::cover(0x3083c0, previous, pc); previous = pc; }
        match pc {
            0x3083c0 => {
                ecx = m.read32(esp.wrapping_add(0x4));
                fa = m.read32(ecx.wrapping_add(0x80));
                fb = ((((0xdi32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x30840f; continue; }
                pc = 0x3083cd; continue;
            }
            0x3083cd => {
                edx = m.read32(0x78400c);
                eax = 0;
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                push!(esi);
                esi = m.read32(ecx.wrapping_add(0x84));
                if (fa == fb) { pc = 0x30840d; continue; }
                pc = 0x3083e0; continue;
            }
            0x3083e0 => {
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x10));
                pc = 0x3083e5; continue;
            }
            0x3083e5 => {
                ecx = m.read32(edx.wrapping_add(0x3c));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x308405; continue; }
                pc = 0x3083ec; continue;
            }
            0x3083ec => {
                pc = 0x3083f0; continue;
            }
            0x3083f0 => {
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x3083fd; continue; }
                pc = 0x3083f4; continue;
            }
            0x3083f4 => {
                fa = m.read32(ecx);
                fb = esi;
                if (fa != fb) { pc = 0x3083fe; continue; }
                pc = 0x3083f8; continue;
            }
            0x3083f8 => {
                fa = m.read32(ecx.wrapping_add(0x4));
                fb = edi;
                if (fa != fb) { pc = 0x3083fe; continue; }
                pc = 0x3083fd; continue;
            }
            0x3083fd => {
                eax = eax.wrapping_add(1);
                pc = 0x3083fe; continue;
            }
            0x3083fe => {
                ecx = m.read32(ecx.wrapping_add(0x14));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3083f0; continue; }
                pc = 0x308405; continue;
            }
            0x308405 => {
                edx = m.read32(edx.wrapping_add(0x40));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3083e5; continue; }
                pc = 0x30840c; continue;
            }
            0x30840c => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x30840d; continue;
            }
            0x30840d => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x30840f => {
                eax = m.read32(ecx.wrapping_add(0x84));
                push!(eax);
                call!(0x308340);
                pc = 0x30841b; continue;
            }
            0x30841b => {
                esp = esp.wrapping_add(0x4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00308420: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f308420<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x308420;
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
        { crate::pose::cover(0x308420, previous, pc); previous = pc; }
        match pc {
            0x308420 => {
                eax = m.read32(esp.wrapping_add(0x4));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x308450; continue; }
                pc = 0x308428; continue;
            }
            0x308428 => {
                edx = m.read32(esp.wrapping_add(0x8));
                push!(esi);
                pc = 0x308430; continue;
            }
            0x308430 => {
                ecx = m.read32(eax);
                ecx = ((ecx as i32).wrapping_mul((0xc4) as i32)) as u32;
                ecx = (((((m.read16(ecx.wrapping_add(0x524f5a)) as i16)) as i32)) as u32);
                esi = m.read32(edx.wrapping_add(ecx.wrapping_mul(0x4)));
                ecx = edx.wrapping_add(ecx.wrapping_mul(0x4));
                esi = esi.wrapping_add(1);
                m.write32(ecx, esi);
                eax = m.read32(eax.wrapping_add(0x14));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x308430; continue; }
                pc = 0x30844f; continue;
            }
            0x30844f => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x308450; continue;
            }
            0x308450 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00308460: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f308460<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x308460;
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
        { crate::pose::cover(0x308460, previous, pc); previous = pc; }
        match pc {
            0x308460 => {
                eax = m.read32(esp.wrapping_add(0x4));
                eax = eax.wrapping_add(eax.wrapping_mul(0x2));
                ecx = m.read32(eax.wrapping_mul(0x8).wrapping_add(0x524be0));
                m.write32(esp.wrapping_add(0x4), ecx);
                tail_to!(0x3d7b0); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00308480: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f308480<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x308480;
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
        { crate::pose::cover(0x308480, previous, pc); previous = pc; }
        match pc {
            0x308480 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f18));
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                fa = ecx;
                fb = esi;
                if (fa != fb) { pc = 0x3084a3; continue; }
                pc = 0x308494; continue;
            }
            0x308494 => {
                m.write32(eax.wrapping_add(0x2f18), 0);
                eax = m.read32(0x7844a8);
                pc = 0x3084a3; continue;
            }
            0x3084a3 => {
                fa = m.read32(eax.wrapping_add(0x2f7c));
                fb = esi;
                if (fa != fb) { pc = 0x3084ba; continue; }
                pc = 0x3084ab; continue;
            }
            0x3084ab => {
                m.write32(eax.wrapping_add(0x2f7c), 0);
                eax = m.read32(0x7844a8);
                pc = 0x3084ba; continue;
            }
            0x3084ba => {
                eax = eax.wrapping_add(0x3068);
                push!(eax);
                push!(esi);
                call!(0x2f9440);
                pc = 0x3084c6; continue;
            }
            0x3084c6 => {
                push!(esi);
                call!(0x2f8bd0);
                pc = 0x3084cc; continue;
            }
            0x3084cc => {
                push!(esi);
                call!(0x2f8d50);
                pc = 0x3084d2; continue;
            }
            0x3084d2 => {
                push!(esi);
                call!(0x2fbd00);
                pc = 0x3084d8; continue;
            }
            0x3084d8 => {
                eax = m.read32(0x78400c);
                esp = esp.wrapping_add(0x14);
                fa = eax;
                fb = esi;
                if (fa != fb) { pc = 0x3084ee; continue; }
                pc = 0x3084e4; continue;
            }
            0x3084e4 => {
                eax = m.read32(esi.wrapping_add(0x40));
                m.write32(0x78400c, eax);
                pc = 0x308507; continue;
            }
            0x3084ee => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30851d; continue; }
                pc = 0x3084f2; continue;
            }
            0x3084f2 => {
                ecx = m.read32(eax.wrapping_add(0x40));
                fa = ecx;
                fb = esi;
                if (fa == fb) { pc = 0x308501; continue; }
                pc = 0x3084f9; continue;
            }
            0x3084f9 => {
                eax = ecx;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3084f2; continue; }
                pc = 0x3084ff; continue;
            }
            0x3084ff => {
                pc = 0x30851d; continue;
            }
            0x308501 => {
                ecx = m.read32(esi.wrapping_add(0x40));
                m.write32(eax.wrapping_add(0x40), ecx);
                pc = 0x308507; continue;
            }
            0x308507 => {
                edx = m.read32(0x784000);
                edx = edx.wrapping_sub(1);
                push!(esi);
                m.write32(0x784000, edx);
                call!(0x307da0);
                pc = 0x30851a; continue;
            }
            0x30851a => {
                esp = esp.wrapping_add(0x4);
                pc = 0x30851d; continue;
            }
            0x30851d => {
                eax = m.read32(esi.wrapping_add(0x3c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x308538; continue; }
                pc = 0x308524; continue;
            }
            0x308524 => {
                edx = m.read32(esi.wrapping_add(0x3c));
                push!(edx);
                push!(esi);
                call!(0x3080b0);
                pc = 0x30852e; continue;
            }
            0x30852e => {
                eax = m.read32(esi.wrapping_add(0x3c));
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x308524; continue; }
                pc = 0x308538; continue;
            }
            0x308538 => {
                eax = m.read32(0x762cc4);
                m.write32(esi.wrapping_add(0x40), eax);
                eax = m.read32(0x783ff8);
                eax = eax.wrapping_add(1);
                m.write32(0x762cc4, esi);
                m.write32(0x783ff8, eax);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00308560: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f308560<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x308560;
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
        { crate::pose::cover(0x308560, previous, pc); previous = pc; }
        match pc {
            0x308560 => {
                eax = m.read32(esp.wrapping_add(0x4));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x308583; continue; }
                pc = 0x308568; continue;
            }
            0x308568 => {
                push!(esi);
                pc = 0x308570; continue;
            }
            0x308570 => {
                esi = m.read32(eax.wrapping_add(0x40));
                push!(eax);
                call!(0x308480);
                pc = 0x308579; continue;
            }
            0x308579 => {
                esp = esp.wrapping_add(0x4);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = esi;
                if (fa != fb) { pc = 0x308570; continue; }
                pc = 0x308582; continue;
            }
            0x308582 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x308583; continue;
            }
            0x308583 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00308AA0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f308aa0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x308aa0;
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
        { crate::pose::cover(0x308aa0, previous, pc); previous = pc; }
        match pc {
            0x308aa0 => {
                ecx = m.read32(esp.wrapping_add(0x4));
                edx = m.read32(0x525640);
                push!(esi);
                esi = m.read32(ecx.wrapping_mul(0x4).wrapping_add(0x52ce00));
                eax = 0;
                fa = edx;
                fb = esi;
                eax = (eax & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00308AC0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f308ac0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x308ac0;
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
        { crate::pose::cover(0x308ac0, previous, pc); previous = pc; }
        match pc {
            0x308ac0 => {
                ecx = m.read32(0x525640);
                eax = 0;
                pc = 0x308ad0; continue;
            }
            0x308ad0 => {
                fa = ecx;
                fb = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x52ce00));
                if (fa == fb) { pc = 0x308ae0; continue; }
                pc = 0x308ad9; continue;
            }
            0x308ad9 => {
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ((((0x32i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x308ad0; continue; }
                pc = 0x308adf; continue;
            }
            0x308adf => {
                break 'run;
            }
            0x308ae0 => {
                ecx = m.read32(esp.wrapping_add(0x4));
                m.write32(ecx.wrapping_add(0x60), eax);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00308AF0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f308af0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x308af0;
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
        { crate::pose::cover(0x308af0, previous, pc); previous = pc; }
        match pc {
            0x308af0 => {
                eax = m.read32(esp.wrapping_add(0x4));
                ecx = m.read32(0x7844a8);
                eax = ((eax as i32).wrapping_mul((0xe4) as i32)) as u32;
                edx = m.read32(eax.wrapping_add(ecx).wrapping_add(0x2e8));
                eax = m.read32(edx.wrapping_mul(0x4).wrapping_add(0x52ce00));
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00308EB0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f308eb0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x308eb0;
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
        { crate::pose::cover(0x308eb0, previous, pc); previous = pc; }
        match pc {
            0x308eb0 => {
                xmm3 = Xmm::scalar(m.read32(0x783de0));
                xmm1.lanes[0] = (((m.read32(0x783ccc)) as i32) as f32).to_bits();
                xmm2.lanes[0] = (((m.read32(0x783de4)) as i32) as f32).to_bits();
                xmm4 = Xmm::scalar(m.read32(0x783dd8));
                xmm4.lanes[0] = sse::addss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm0 = xmm1;
                xmm0.lanes[0] = sse::divss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                m.write32(0x783dc0, xmm4.lanes[0]);
                xmm4 = Xmm::scalar(m.read32(0x783cc0));
                xmm4.lanes[0] = sse::addss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::divss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(0x783ddc, xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                xmm2.lanes[0] = sse::divss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = xmm0;
                xmm3.lanes[0] = sse::divss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::divss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(0x783dcc, xmm4.lanes[0]);
                m.write32(0x783f2c, xmm2.lanes[0]);
                m.write32(0x783f28, xmm1.lanes[0]);
                m.write32(0x783dd4, xmm3.lanes[0]);
                m.write32(0x783f20, xmm0.lanes[0]);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0030AD90: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f30ad90<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x30ad90;
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
        { crate::pose::cover(0x30ad90, previous, pc); previous = pc; }
        match pc {
            0x30ad90 => {
                eax = m.read32(esp.wrapping_add(0xc));
                push!(esi);
                esi = m.read32(esp.wrapping_add(0xc));
                fpu.push((m.read32(esi) as i32 as f64));
                push!(eax);
                m.write32(esp.wrapping_add(0x10), (fpu.st(0) as f32).to_bits()); fpu.pop();
                call!(0x40620);
                pc = 0x30ada5; continue;
            }
            0x30ada5 => {
                fpu.push((f32::from_bits(m.read32(0x7ba188)) as f64));
                esp = esp.wrapping_add(0x4);
                { let value=round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4785e4)) as f64),fpu.control);fpu.stack[((fpu.top+0)&7) as usize]=value; }
                { let value=round_pc(fpu.st(1) + fpu.st(0),fpu.control);fpu.stack[((fpu.top+1)&7) as usize]=value; } fpu.pop();
                { let value=round_pc((f32::from_bits(m.read32(esp.wrapping_add(0xc))) as f64) - fpu.st(0),fpu.control);fpu.stack[((fpu.top+0)&7) as usize]=value; }
                call!(0x3c848c);
                pc = 0x30adbf; continue;
            }
            0x30adbf => {
                xmm1.lanes[0] = (((m.read32(esp.wrapping_add(0x14))) as i32) as f32).to_bits();
                edx = m.read32(esp.wrapping_add(0x8));
                m.write32(esi, eax);
                xmm0 = Xmm::scalar(m.read32(0x7ba038));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x4760cc))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475cd4))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                ecx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                m.write32(edx, ecx);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0030ADF0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f30adf0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x30adf0;
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
        { crate::pose::cover(0x30adf0, previous, pc); previous = pc; }
        match pc {
            0x30adf0 => {
                push!(ecx);
                fa = m.read32(0x7e7260);
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x14));
                push!(esi);
                if (fa != fb) { pc = 0x30ae0d; continue; }
                pc = 0x30ae00; continue;
            }
            0x30ae00 => {
                call!(0x405c0);
                pc = 0x30ae05; continue;
            }
            0x30ae05 => {
                { let value=round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x475cd4)) as f64),fpu.control);fpu.stack[((fpu.top+0)&7) as usize]=value; }
                pc = 0x30ae12; continue;
            }
            0x30ae0d => {
                call!(0x405c0);
                pc = 0x30ae12; continue;
            }
            0x30ae12 => {
                xmm1 = Xmm::scalar(m.read32(0x7ba188));
                m.write32(esp.wrapping_add(0x8), (fpu.st(0) as f32).to_bits()); fpu.pop();
                xmm0.lanes[0] = (((m.read32(esp.wrapping_add(0x1c))) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x8)))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x4785e4))).to_bits();
                ecx = m.read32(esp.wrapping_add(0x10));
                esp = esp.wrapping_add(0x4);
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                eax = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(esi);
                m.write32(ecx, eax);
                call!(0x40620);
                pc = 0x30ae49; continue;
            }
            0x30ae49 => {
                { let value=round_pc((f32::from_bits(m.read32(0x47db00)) as f64) - fpu.st(0),fpu.control);fpu.stack[((fpu.top+0)&7) as usize]=value; }
                esi = m.read32(esp.wrapping_add(0x14));
                esp = esp.wrapping_add(0x4);
                { let value=round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x475cd4)) as f64),fpu.control);fpu.stack[((fpu.top+0)&7) as usize]=value; }
                { let value=round_pc(fpu.st(0) + (m.read32(esi) as i32 as f64),fpu.control);fpu.stack[((fpu.top+0)&7) as usize]=value; }
                call!(0x3c848c);
                pc = 0x30ae63; continue;
            }
            0x30ae63 => {
                m.write32(esi, eax);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0030AE70: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f30ae70<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x30ae70;
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
        { crate::pose::cover(0x30ae70, previous, pc); previous = pc; }
        match pc {
            0x30ae70 => {
                fa = m.read32(esp.wrapping_add(0x14));
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x30ae9b; continue; }
                pc = 0x30ae77; continue;
            }
            0x30ae77 => {
                eax = m.read32(esp.wrapping_add(0xc));
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                fpu.push((m.read32(esi) as i32 as f64));
                push!(eax);
                m.write32(esp.wrapping_add(0x1c), (fpu.st(0) as f32).to_bits()); fpu.pop();
                call!(0x405c0);
                pc = 0x30ae8c; continue;
            }
            0x30ae8c => {
                { let value=round_pc((f32::from_bits(m.read32(esp.wrapping_add(0x1c))) as f64) - fpu.st(0),fpu.control);fpu.stack[((fpu.top+0)&7) as usize]=value; }
                esp = esp.wrapping_add(0x4);
                call!(0x3c848c);
                pc = 0x30ae98; continue;
            }
            0x30ae98 => {
                m.write32(esi, eax);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x30ae9b; continue;
            }
            0x30ae9b => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00311900: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f311900<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x311900;
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
        { crate::pose::cover(0x311900, previous, pc); previous = pc; }
        match pc {
            0x311900 => {
                eax = m.read32(esp.wrapping_add(0x4));
                m.write32(0x78402c, eax);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x003119B0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3119b0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3119b0;
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
        { crate::pose::cover(0x3119b0, previous, pc); previous = pc; }
        match pc {
            0x3119b0 => {
                eax = m.read32(esp.wrapping_add(0x4));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3119c8; continue; }
                pc = 0x3119b8; continue;
            }
            0x3119b8 => {
                eax = m.read32(eax.wrapping_add(0x2c));
                push!(0xffffffffu32);
                push!(0);
                push!(eax);
                call!(0x79860);
                pc = 0x3119c5; continue;
            }
            0x3119c5 => {
                esp = esp.wrapping_add(0xc);
                pc = 0x3119c8; continue;
            }
            0x3119c8 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x25c));
                ecx = (ecx & 0xfffffff7u32);
                push!(0);
                m.write32(eax.wrapping_add(0x25c), ecx);
                call!(0x335840);
                pc = 0x3119e3; continue;
            }
            0x3119e3 => {
                push!(0x1);
                call!(0x335840);
                pc = 0x3119ea; continue;
            }
            0x3119ea => {
                push!(0x5);
                call!(0x335840);
                pc = 0x3119f1; continue;
            }
            0x3119f1 => {
                push!(0x8);
                call!(0x335840);
                pc = 0x3119f8; continue;
            }
            0x3119f8 => {
                push!(0x2);
                call!(0x3356f0);
                pc = 0x3119ff; continue;
            }
            0x3119ff => {
                push!(0x3);
                call!(0x3356f0);
                pc = 0x311a06; continue;
            }
            0x311a06 => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x200));
                esp = esp.wrapping_add(0x18);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x311a23; continue; }
                pc = 0x311a19; continue;
            }
            0x311a19 => {
                push!(0x4);
                call!(0x3356f0);
                pc = 0x311a20; continue;
            }
            0x311a20 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x311a23; continue;
            }
            0x311a23 => {
                push!(0xffffffffu32);
                push!(0);
                call!(0x30cd40);
                pc = 0x311a2c; continue;
            }
            0x311a2c => {
                call!(0x73c80);
                pc = 0x311a31; continue;
            }
            0x311a31 => {
                push!(0x100);
                push!(0);
                call!(0x42830);
                pc = 0x311a3d; continue;
            }
            0x311a3d => {
                push!(0x100);
                push!(0x1);
                call!(0x42830);
                pc = 0x311a49; continue;
            }
            0x311a49 => {
                push!(0x100);
                push!(0x2);
                call!(0x42830);
                pc = 0x311a55; continue;
            }
            0x311a55 => {
                push!(0x100);
                push!(0x3);
                call!(0x42830);
                pc = 0x311a61; continue;
            }
            0x311a61 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x238));
                ecx = (ecx | 0x4000);
                esp = esp.wrapping_add(0x28);
                m.write32(eax.wrapping_add(0x238), ecx);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00311FB0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f311fb0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x311fb0;
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
        { crate::pose::cover(0x311fb0, previous, pc); previous = pc; }
        match pc {
            0x311fb0 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                call!(0x311e90);
                pc = 0x311fba; continue;
            }
            0x311fba => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00312570: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f312570<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x312570;
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
        { crate::pose::cover(0x312570, previous, pc); previous = pc; }
        match pc {
            0x312570 => {
                eax = m.read32(esp.wrapping_add(0x4));
                push!(0);
                push!(eax);
                call!(0x71880);
                pc = 0x31257c; continue;
            }
            0x31257c => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31259f; continue; }
                pc = 0x312583; continue;
            }
            0x312583 => {
                edx = m.read32(0x7844a8);
                eax = m.read32(edx.wrapping_add(0x230));
                ecx = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ecx = (ecx & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                m.write32(esp.wrapping_add(0x4), ecx);
                tail_to!(0x46270); break 'run;  // direct tail jump
            }
            0x31259f => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00312810: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f312810<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x312810;
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
        { crate::pose::cover(0x312810, previous, pc); previous = pc; }
        match pc {
            0x312810 => {
                push!(esi);
                push!(edi);
                push!(0xc77668);
                esi = 0;
                call!(0x3365f0);
                pc = 0x31281e; continue;
            }
            0x31281e => {
                push!(eax);
                call!(0x32a5b0);
                pc = 0x312824; continue;
            }
            0x312824 => {
                edx = m.read32(0x7844a8);
                ecx = ((m.read8(eax.wrapping_add(0x16))) as u8 as u32);
                edi = m.read32(edx.wrapping_add(0x11860));
                eax = eax.wrapping_add(0x16);
                esp = esp.wrapping_add(0x8);
                fa = ecx;
                fb = edi;
                if (fa != fb) { pc = 0x312874; continue; }
                pc = 0x31283e; continue;
            }
            0x31283e => {
                ecx = 0;
                edx = edx.wrapping_add(0x1186a);
                eax = eax.wrapping_add(0x5);
                pc = 0x312850; continue;
            }
            0x312850 => {
                edi = (edi & 0xffff_0000) | ((((m.read16(eax.wrapping_add(((0x3i32.wrapping_neg()) as u32)))) as u32)) & 0xffff);
                fa = ((((((edi) as u16))) as u32) & 0xffffu32);
                fb = ((((m.read16(edx.wrapping_add(((0x6i32.wrapping_neg()) as u32))))) as u32) & 0xffffu32);
                if (fa != fb) { pc = 0x312874; continue; }
                pc = 0x31285a; continue;
            }
            0x31285a => {
                edi = (edi & 0xffff_0000) | (((((((m.read8(eax) as i8)) as i32)) as u32)) & 0xffff);
                fa = ((((((edi) as u16))) as u32) & 0xffffu32);
                fb = ((((m.read16(edx))) as u32) & 0xffffu32);
                if (fa != fb) { pc = 0x312874; continue; }
                pc = 0x312863; continue;
            }
            0x312863 => {
                ecx = ecx.wrapping_add(1);
                eax = eax.wrapping_add(0x4);
                edx = edx.wrapping_add(0x8);
                fa = ecx;
                fb = ((((0xai32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x312850; continue; }
                pc = 0x31286f; continue;
            }
            0x31286f => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = esi;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x312874 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0x1;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00312880: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f312880<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x312880;
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
        { crate::pose::cover(0x312880, previous, pc); previous = pc; }
        match pc {
            0x312880 => {
                push!(esi);
                push!(edi);
                push!(0xc77668);
                edi = 0;
                call!(0x3365f0);
                pc = 0x31288e; continue;
            }
            0x31288e => {
                push!(eax);
                call!(0x32a5b0);
                pc = 0x312894; continue;
            }
            0x312894 => {
                esi = m.read32(0x7844a8);
                ecx = ((m.read8(eax.wrapping_add(0x42))) as u8 as u32);
                edx = m.read32(esi.wrapping_add(0x11908));
                eax = eax.wrapping_add(0x42);
                esp = esp.wrapping_add(0x8);
                fa = ecx;
                fb = edx;
                if (fa == fb) { pc = 0x3128b6; continue; }
                pc = 0x3128ae; continue;
            }
            0x3128ae => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0x1;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x3128b6 => {
                edx = eax.wrapping_add(0x2);
                ecx = 0;
                eax = esi.wrapping_add(0x1190c);
                pc = 0x3128c1; continue;
            }
            0x3128c1 => {
                esi = (((((m.read16(edx) as i16)) as i32)) as u32);
                fa = esi;
                fb = m.read32(eax);
                if (fa != fb) { pc = 0x3128d9; continue; }
                pc = 0x3128c8; continue;
            }
            0x3128c8 => {
                ecx = ecx.wrapping_add(1);
                eax = eax.wrapping_add(0x4);
                edx = edx.wrapping_add(0x2);
                fa = ecx;
                fb = ((((0xfi32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x3128c1; continue; }
                pc = 0x3128d4; continue;
            }
            0x3128d4 => {
                eax = edi;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x3128d9 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0x1;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00312950: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f312950<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x312950;
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
        { crate::pose::cover(0x312950, previous, pc); previous = pc; }
        match pc {
            0x312950 => {
                fa = m.read32(0x79094c);
                fb = ((((0x66i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x312964; continue; }
                pc = 0x312959; continue;
            }
            0x312959 => {
                eax = m.read32(0x7844a8);
                eax = eax.wrapping_add(0x11860);
                break 'run;
            }
            0x312964 => {
                push!(edi);
                eax = 0;
                ecx = 0x29;
                edi = 0x762dc0;
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), eax); } edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep stosd, DF from entry.eflags
                ecx = 0;
                eax = m.read32(0x784014);
                push!(0xc77668);
                push!(eax);
                call!(0x32a5b0);
                pc = 0x312983; continue;
            }
            0x312983 => {
                edx = ((m.read8(eax.wrapping_add(0x16))) as u8 as u32);
                eax = eax.wrapping_add(0x16);
                esp = esp.wrapping_add(0x8);
                ecx = 0;
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(0x762dc0, edx);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                if (fas <= fbs) { pc = 0x3129c6; continue; }
                pc = 0x31299a; continue;
            }
            0x31299a => {
                eax = eax.wrapping_add(0x5);
                pc = 0x3129a0; continue;
            }
            0x3129a0 => {
                edx = (edx & 0xffff_0000) | ((((m.read16(eax.wrapping_add(((0x3i32.wrapping_neg()) as u32)))) as u32)) & 0xffff);
                write16(m, ecx.wrapping_mul(0x8).wrapping_add(0x762dc4), (((((edx) as u16)) as u32)) as u16);
                edx = (edx & 0xffff_0000) | (((((((m.read8(eax) as i8)) as i32)) as u32)) & 0xffff);
                write16(m, ecx.wrapping_mul(0x8).wrapping_add(0x762dca), (((((edx) as u16)) as u32)) as u16);
                edx = m.read32(0x762dc0);
                ecx = ecx.wrapping_add(1);
                eax = eax.wrapping_add(0x4);
                fa = ecx;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x3129a0; continue; }
                pc = 0x3129c6; continue;
            }
            0x3129c6 => {
                eax = 0x762dc0;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x003129D0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3129d0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3129d0;
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
        { crate::pose::cover(0x3129d0, previous, pc); previous = pc; }
        match pc {
            0x3129d0 => {
                eax = m.read32(0x784024);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00312D20: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f312d20<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x312d20;
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
        { crate::pose::cover(0x312d20, previous, pc); previous = pc; }
        match pc {
            0x312d20 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                push!(esi);
                call!(0x71de0);
                pc = 0x312d2b; continue;
            }
            0x312d2b => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x312d50; continue; }
                pc = 0x312d32; continue;
            }
            0x312d32 => {
                eax = m.read32(esi.wrapping_add(0x2c));
                xmm0 = Xmm::default();
                push!(0xffffffffu32);
                push!(0x52d4b4);
                push!(eax);
                m.write32(0x762d70, xmm0.lanes[0]);
                call!(0x79860);
                pc = 0x312d4d; continue;
            }
            0x312d4d => {
                esp = esp.wrapping_add(0xc);
                pc = 0x312d50; continue;
            }
            0x312d50 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00312E50: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f312e50<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x312e50;
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
        { crate::pose::cover(0x312e50, previous, pc); previous = pc; }
        match pc {
            0x312e50 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                eax = m.read32(esi.wrapping_add(0x18c));
                fa = eax;
                fb = ((((0xbi32)) as u32) & 0xffffffffu32);
                if (fa > fb) { pc = 0x312fd6; continue; }
                pc = 0x312e64; continue;
            }
            0x312e64 => {
                jt = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x312fd8));
                if (jt == 0x312e6bu32) { pc = 0x312e6b; continue; }
                if (jt == 0x312e80u32) { pc = 0x312e80; continue; }
                if (jt == 0x312e96u32) { pc = 0x312e96; continue; }
                if (jt == 0x312ec3u32) { pc = 0x312ec3; continue; }
                if (jt == 0x312eebu32) { pc = 0x312eeb; continue; }
                if (jt == 0x312f14u32) { pc = 0x312f14; continue; }
                if (jt == 0x312f19u32) { pc = 0x312f19; continue; }
                if (jt == 0x312f24u32) { pc = 0x312f24; continue; }
                if (jt == 0x312f39u32) { pc = 0x312f39; continue; }
                if (jt == 0x312f4eu32) { pc = 0x312f4e; continue; }
                if (jt == 0x312f81u32) { pc = 0x312f81; continue; }
                if (jt == 0x312f97u32) { pc = 0x312f97; continue; }
                tail_to!(jt); break 'run;  // indirect tail jump, target outside the switch
            }
            0x312e6b => {
                eax = m.read32(0x7842a8);
                push!(0x52d494);
                push!(eax);
                call!(0x79b70);
                pc = 0x312e7b; continue;
            }
            0x312e7b => {
                esp = esp.wrapping_add(0x8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x312e80 => {
                ecx = m.read32(0x7842a8);
                push!(0x52d474);
                push!(ecx);
                call!(0x79b70);
                pc = 0x312e91; continue;
            }
            0x312e91 => {
                esp = esp.wrapping_add(0x8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x312e96 => {
                push!(0x1575);
                call!(0x2f79e0);
                pc = 0x312ea0; continue;
            }
            0x312ea0 => {
                esp = esp.wrapping_add(0x4);
                push!(0);
                push!(0x311aa0);
                push!(0xae1);
                call!(0x3d7b0);
                pc = 0x312eb4; continue;
            }
            0x312eb4 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(esi);
                call!(0x7a080);
                pc = 0x312ebe; continue;
            }
            0x312ebe => {
                esp = esp.wrapping_add(0x10);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x312ec3 => {
                edx = m.read32(0x784014);
                push!(edx);
                call!(0x337760);
                pc = 0x312ecf; continue;
            }
            0x312ecf => {
                eax = m.read32(0x784014);
                push!(0x1);
                push!(eax);
                call!(0x337600);
                pc = 0x312edc; continue;
            }
            0x312edc => {
                push!(esi);
                call!(0x3119b0);
                pc = 0x312ee2; continue;
            }
            0x312ee2 => {
                esp = esp.wrapping_add(0x10);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                tail_to!(0x300580); break 'run;  // direct tail jump
            }
            0x312eeb => {
                ecx = m.read32(0x784014);
                push!(ecx);
                call!(0x337760);
                pc = 0x312ef7; continue;
            }
            0x312ef7 => {
                edx = m.read32(0x784014);
                push!(0x2);
                push!(edx);
                call!(0x337600);
                pc = 0x312f05; continue;
            }
            0x312f05 => {
                push!(esi);
                call!(0x3119b0);
                pc = 0x312f0b; continue;
            }
            0x312f0b => {
                esp = esp.wrapping_add(0x10);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                tail_to!(0x300580); break 'run;  // direct tail jump
            }
            0x312f14 => {
                call!(0x308f40);
                pc = 0x312f19; continue;
            }
            0x312f19 => {
                push!(esi);
                call!(0x3119b0);
                pc = 0x312f1f; continue;
            }
            0x312f1f => {
                esp = esp.wrapping_add(0x4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x312f24 => {
                eax = m.read32(0x7842a8);
                push!(0x52d484);
                push!(eax);
                call!(0x79b70);
                pc = 0x312f34; continue;
            }
            0x312f34 => {
                esp = esp.wrapping_add(0x8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x312f39 => {
                eax = m.read32(0x7844a8);
                m.write32(eax.wrapping_add(0x238), (m.read32(eax.wrapping_add(0x238)) | 0x400));
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                tail_to!(0x32b1d0); break 'run;  // direct tail jump
            }
            0x312f4e => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x238));
                ecx = (ecx | 0x400);
                push!(0x4766d8);
                m.write32(eax.wrapping_add(0x238), ecx);
                call!(0x3c230);
                pc = 0x312f6f; continue;
            }
            0x312f6f => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                if (fa == fb) { pc = 0x312f7c; continue; }
                pc = 0x312f77; continue;
            }
            0x312f77 => {
                tail_to!(0x338020); break 'run;  // direct tail jump
            }
            0x312f7c => {
                tail_to!(0x32b240); break 'run;  // direct tail jump
            }
            0x312f81 => {
                ecx = m.read32(0x7842a8);
                push!(0x52d510);
                push!(ecx);
                call!(0x79b70);
                pc = 0x312f92; continue;
            }
            0x312f92 => {
                esp = esp.wrapping_add(0x8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x312f97 => {
                eax = m.read32(0x78407c);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x312fcd; continue; }
                pc = 0x312fa0; continue;
            }
            0x312fa0 => {
                push!(0x1575);
                call!(0x2f79e0);
                pc = 0x312faa; continue;
            }
            0x312faa => {
                esp = esp.wrapping_add(0x4);
                push!(0);
                push!(0x312df0);
                push!(0xafa);
                call!(0x3d7b0);
                pc = 0x312fbe; continue;
            }
            0x312fbe => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(esi);
                call!(0x7a080);
                pc = 0x312fc8; continue;
            }
            0x312fc8 => {
                esp = esp.wrapping_add(0x10);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x312fcd => {
                push!(esi);
                call!(0x312df0);
                pc = 0x312fd3; continue;
            }
            0x312fd3 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x312fd6; continue;
            }
            0x312fd6 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00314210: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f314210<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x314210;
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
        { crate::pose::cover(0x314210, previous, pc); previous = pc; }
        match pc {
            0x314210 => {
                push!(ebx);
                push!(esi);
                push!(edi);
                call!(0x312950);
                pc = 0x314218; continue;
            }
            0x314218 => {
                ecx = m.read32(esp.wrapping_add(0x10));
                edx = (edx & 0xffff_0000) | ((((m.read16(ecx)) as u32)) & 0xffff);
                write16(m, eax.wrapping_add(0x4), (((((edx) as u16)) as u32)) as u16);
                edx = (edx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(0x18))) as u32)) & 0xffff);
                write16(m, eax.wrapping_add(0xa), (((((edx) as u16)) as u32)) as u16);
                esi = m.read32(ecx);
                edx = 0;
                edi = (edi | 0xffffffffu32);
                fa = esi;
                fb = edi;
                edx = (edx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                esi = edx;
                edx = (edx & 0xffff_ff00) | ((((m.read8(ecx.wrapping_add(0x8))) as u32)) & 0xff);
                m.write8(eax.wrapping_add(0x7), (((((edx) as u8)) as u32)) as u8);
                edx = (edx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(0x1c))) as u32)) & 0xffff);
                write16(m, eax.wrapping_add(0xc), (((((edx) as u16)) as u32)) as u16);
                edx = (edx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(0x34))) as u32)) & 0xffff);
                write16(m, eax.wrapping_add(0x12), (((((edx) as u16)) as u32)) as u16);
                ebx = m.read32(ecx.wrapping_add(0x1c));
                edx = 0;
                fa = ebx;
                fb = edi;
                edx = (edx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                esi = esi.wrapping_add(edx);
                edx = (edx & 0xffff_ff00) | ((((m.read8(ecx.wrapping_add(0x24))) as u32)) & 0xff);
                m.write8(eax.wrapping_add(0xf), (((((edx) as u8)) as u32)) as u8);
                edx = (edx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(0x38))) as u32)) & 0xffff);
                write16(m, eax.wrapping_add(0x14), (((((edx) as u16)) as u32)) as u16);
                edx = (edx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(0x50))) as u32)) & 0xffff);
                write16(m, eax.wrapping_add(0x1a), (((((edx) as u16)) as u32)) as u16);
                ebx = m.read32(ecx.wrapping_add(0x38));
                edx = 0;
                fa = ebx;
                fb = edi;
                edx = (edx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                esi = esi.wrapping_add(edx);
                edx = (edx & 0xffff_ff00) | ((((m.read8(ecx.wrapping_add(0x40))) as u32)) & 0xff);
                m.write8(eax.wrapping_add(0x17), (((((edx) as u8)) as u32)) as u8);
                edx = (edx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(0x54))) as u32)) & 0xffff);
                write16(m, eax.wrapping_add(0x1c), (((((edx) as u16)) as u32)) as u16);
                edx = (edx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(0x6c))) as u32)) & 0xffff);
                write16(m, eax.wrapping_add(0x22), (((((edx) as u16)) as u32)) as u16);
                ebx = m.read32(ecx.wrapping_add(0x54));
                edx = 0;
                fa = ebx;
                fb = edi;
                edx = (edx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                esi = esi.wrapping_add(edx);
                edx = (edx & 0xffff_ff00) | ((((m.read8(ecx.wrapping_add(0x5c))) as u32)) & 0xff);
                m.write8(eax.wrapping_add(0x1f), (((((edx) as u8)) as u32)) as u8);
                edx = (edx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(0x70))) as u32)) & 0xffff);
                write16(m, eax.wrapping_add(0x24), (((((edx) as u16)) as u32)) as u16);
                edx = (edx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(0x88))) as u32)) & 0xffff);
                write16(m, eax.wrapping_add(0x2a), (((((edx) as u16)) as u32)) as u16);
                ebx = m.read32(ecx.wrapping_add(0x70));
                edx = 0;
                fa = ebx;
                fb = edi;
                edx = (edx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                esi = esi.wrapping_add(edx);
                edx = (edx & 0xffff_ff00) | ((((m.read8(ecx.wrapping_add(0x78))) as u32)) & 0xff);
                m.write8(eax.wrapping_add(0x27), (((((edx) as u8)) as u32)) as u8);
                edx = (edx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(0x8c))) as u32)) & 0xffff);
                write16(m, eax.wrapping_add(0x2c), (((((edx) as u16)) as u32)) as u16);
                edx = (edx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(0xa4))) as u32)) & 0xffff);
                write16(m, eax.wrapping_add(0x32), (((((edx) as u16)) as u32)) as u16);
                ebx = m.read32(ecx.wrapping_add(0x8c));
                edx = 0;
                fa = ebx;
                fb = edi;
                edx = (edx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                esi = esi.wrapping_add(edx);
                edx = (edx & 0xffff_ff00) | ((((m.read8(ecx.wrapping_add(0x94))) as u32)) & 0xff);
                m.write8(eax.wrapping_add(0x2f), (((((edx) as u8)) as u32)) as u8);
                edx = (edx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(0xa8))) as u32)) & 0xffff);
                write16(m, eax.wrapping_add(0x34), (((((edx) as u16)) as u32)) as u16);
                edx = (edx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(0xc0))) as u32)) & 0xffff);
                write16(m, eax.wrapping_add(0x3a), (((((edx) as u16)) as u32)) as u16);
                ebx = m.read32(ecx.wrapping_add(0xa8));
                edx = 0;
                fa = ebx;
                fb = edi;
                edx = (edx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                esi = esi.wrapping_add(edx);
                edx = (edx & 0xffff_ff00) | ((((m.read8(ecx.wrapping_add(0xb0))) as u32)) & 0xff);
                m.write8(eax.wrapping_add(0x37), (((((edx) as u8)) as u32)) as u8);
                edx = (edx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(0xc4))) as u32)) & 0xffff);
                write16(m, eax.wrapping_add(0x3c), (((((edx) as u16)) as u32)) as u16);
                edx = (edx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(0xdc))) as u32)) & 0xffff);
                write16(m, eax.wrapping_add(0x42), (((((edx) as u16)) as u32)) as u16);
                ebx = m.read32(ecx.wrapping_add(0xc4));
                edx = 0;
                fa = ebx;
                fb = edi;
                edx = (edx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                esi = esi.wrapping_add(edx);
                edx = (edx & 0xffff_ff00) | ((((m.read8(ecx.wrapping_add(0xcc))) as u32)) & 0xff);
                m.write8(eax.wrapping_add(0x3f), (((((edx) as u8)) as u32)) as u8);
                edx = (edx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(0xe0))) as u32)) & 0xffff);
                write16(m, eax.wrapping_add(0x44), (((((edx) as u16)) as u32)) as u16);
                edx = (edx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(0xf8))) as u32)) & 0xffff);
                write16(m, eax.wrapping_add(0x4a), (((((edx) as u16)) as u32)) as u16);
                ebx = m.read32(ecx.wrapping_add(0xe0));
                edx = 0;
                fa = ebx;
                fb = edi;
                edx = (edx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                esi = esi.wrapping_add(edx);
                edx = (edx & 0xffff_ff00) | ((((m.read8(ecx.wrapping_add(0xe8))) as u32)) & 0xff);
                m.write8(eax.wrapping_add(0x47), (((((edx) as u8)) as u32)) as u8);
                edx = (edx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(0xfc))) as u32)) & 0xffff);
                write16(m, eax.wrapping_add(0x4c), (((((edx) as u16)) as u32)) as u16);
                edx = (edx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(0x114))) as u32)) & 0xffff);
                write16(m, eax.wrapping_add(0x52), (((((edx) as u16)) as u32)) as u16);
                ebx = m.read32(ecx.wrapping_add(0xfc));
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(ecx.wrapping_add(0x104))) as u32)) & 0xff);
                edx = 0;
                fa = ebx;
                fb = edi;
                edx = (edx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write8(eax.wrapping_add(0x4f), (((((ecx) as u8)) as u32)) as u8);
                esi = esi.wrapping_add(edx);
                m.write32(eax, esi);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x003143C0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3143c0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3143c0;
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
        { crate::pose::cover(0x3143c0, previous, pc); previous = pc; }
        match pc {
            0x3143c0 => {
                xmm0.lanes[0] = (((m.read32(0x4e8cec)) as i32) as f32).to_bits();
                xmm1 = Xmm::scalar(m.read32(0x475c78));
                eax = m.read32(esp.wrapping_add(0x4));
                xmm1.lanes[0] = sse::divss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x4e7948))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x762d70))).to_bits();
                push!(eax);
                m.write32(0x762d70, xmm1.lanes[0]);
                call!(0x71de0);
                pc = 0x3143f6; continue;
            }
            0x3143f6 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x314413; continue; }
                pc = 0x3143fd; continue;
            }
            0x3143fd => {
                xmm0 = Xmm::scalar(m.read32(0x762d70));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x475d10)) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x314413; continue; }
                pc = 0x31440e; continue;
            }
            0x31440e => {
                tail_to!(0x312d60); break 'run;  // direct tail jump
            }
            0x314413 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00314B20: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f314b20<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x314b20;
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
        { crate::pose::cover(0x314b20, previous, pc); previous = pc; }
        match pc {
            0x314b20 => {
                push!(0);
                push!(0);
                push!(0xaa8);
                m.write32(0x784010, 0x10);
                call!(0x3d7b0);
                pc = 0x314b38; continue;
            }
            0x314b38 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x1);
                call!(0x74a30);
                pc = 0x314b43; continue;
            }
            0x314b43 => {
                esp = esp.wrapping_add(0x10);
                push!(0);
                push!(0x2);
                push!(0xaaa);
                call!(0x3d7b0);
                pc = 0x314b54; continue;
            }
            0x314b54 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0);
                call!(0x74a30);
                pc = 0x314b5f; continue;
            }
            0x314b5f => {
                esp = esp.wrapping_add(0x10);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00314C90: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f314c90<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x314c90;
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
        { crate::pose::cover(0x314c90, previous, pc); previous = pc; }
        match pc {
            0x314c90 => {
                eax = m.read32(0x7ba9e0);
                eax = ((eax as i32).wrapping_mul((0x1e0) as i32)) as u32;
                ecx = m.read32(eax.wrapping_add(0x7ba26c));
                push!(esi);
                push!(ecx);
                call!(0x77470);
                pc = 0x314ca8; continue;
            }
            0x314ca8 => {
                edx = m.read32(0x7844a8);
                esi = eax;
                eax = m.read32(edx.wrapping_add(0x25c));
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0x4044088i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) != 0) { pc = 0x314ce9; continue; }
                pc = 0x314cc0; continue;
            }
            0x314cc0 => {
                eax = m.read32(0x7ba9e0);
                eax = ((eax as i32).wrapping_mul((0x1e0) as i32)) as u32;
                ecx = m.read32(eax.wrapping_add(0x7ba26c));
                push!(ecx);
                call!(0x77470);
                pc = 0x314cd7; continue;
            }
            0x314cd7 => {
                ecx = m.read32(0x7842a8);
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ecx;
                if (fa != fb) { pc = 0x314ce9; continue; }
                pc = 0x314ce4; continue;
            }
            0x314ce4 => {
                call!(0x314b20);
                pc = 0x314ce9; continue;
            }
            0x314ce9 => {
                edx = m.read32(0x7844a8);
                fa = m.read32(edx.wrapping_add(0x25c));
                fb = ((((0x4044088i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) != 0) { pc = 0x314d1f; continue; }
                pc = 0x314cfb; continue;
            }
            0x314cfb => {
                eax = m.read32(0x7ba9e0);
                eax = ((eax as i32).wrapping_mul((0x1e0) as i32)) as u32;
                ecx = m.read32(eax.wrapping_add(0x7ba26c));
                push!(ecx);
                call!(0x77470);
                pc = 0x314d12; continue;
            }
            0x314d12 => {
                ecx = m.read32(0x7842a8);
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ecx;
                if (fa == fb) { pc = 0x314d51; continue; }
                pc = 0x314d1f; continue;
            }
            0x314d1f => {
                eax = m.read32(esi.wrapping_add(0x4));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x314d51; continue; }
                pc = 0x314d26; continue;
            }
            0x314d26 => {
                call!(0x314ac0);
                pc = 0x314d2b; continue;
            }
            0x314d2b => {
                eax = m.read32(esi.wrapping_add(0x198));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x314d51; continue; }
                pc = 0x314d35; continue;
            }
            0x314d35 => {
                eax = m.read32(eax.wrapping_add(0x18));
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x314d51; continue; }
                pc = 0x314d3d; continue;
            }
            0x314d3d => {
                fa = eax;
                fb = ((((0x110i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x314d51; continue; }
                pc = 0x314d44; continue;
            }
            0x314d44 => {
                push!(0);
                push!(0);
                push!(esi);
                call!(0x74910);
                pc = 0x314d4e; continue;
            }
            0x314d4e => {
                esp = esp.wrapping_add(0xc);
                pc = 0x314d51; continue;
            }
            0x314d51 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00314D60: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f314d60<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x314d60;
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
        { crate::pose::cover(0x314d60, previous, pc); previous = pc; }
        match pc {
            0x314d60 => {
                eax = m.read32(0x783b94);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00314D70: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f314d70<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x314d70;
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
        { crate::pose::cover(0x314d70, previous, pc); previous = pc; }
        match pc {
            0x314d70 => {
                eax = m.read32(0x7ba9e0);
                eax = ((eax as i32).wrapping_mul((0x1e0) as i32)) as u32;
                ecx = m.read32(eax.wrapping_add(0x7ba26c));
                push!(esi);
                push!(ecx);
                call!(0x77470);
                pc = 0x314d88; continue;
            }
            0x314d88 => {
                esi = eax;
                eax = m.read32(0x783b94);
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x314dc0; continue; }
                pc = 0x314d96; continue;
            }
            0x314d96 => {
                edx = m.read32(esp.wrapping_add(0x8));
                eax = m.read32(edx.wrapping_mul(0x4).wrapping_add(0x52d530));
                push!(0);
                push!(eax);
                m.write32(0x783b94, 0x1);
                call!(0x3d7b0);
                pc = 0x314db3; continue;
            }
            0x314db3 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(esi);
                call!(0x7a270);
                pc = 0x314dbd; continue;
            }
            0x314dbd => {
                esp = esp.wrapping_add(0xc);
                pc = 0x314dc0; continue;
            }
            0x314dc0 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00314DD0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f314dd0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x314dd0;
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
        { crate::pose::cover(0x314dd0, previous, pc); previous = pc; }
        match pc {
            0x314dd0 => {
                call!(0x317c70);
                pc = 0x314dd5; continue;
            }
            0x314dd5 => {
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x314dfe; continue; }
                pc = 0x314dda; continue;
            }
            0x314dda => {
                fa = eax;
                fb = ((((0x50i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x314dfe; continue; }
                pc = 0x314ddf; continue;
            }
            0x314ddf => {
                eax = ((eax as i32).wrapping_mul((0xc4) as i32)) as u32;
                eax = m.read32(eax.wrapping_add(0x525014));
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x314dfe; continue; }
                pc = 0x314df0; continue;
            }
            0x314df0 => {
                fa = eax;
                fb = ((((0x110i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x314dfe; continue; }
                pc = 0x314df7; continue;
            }
            0x314df7 => {
                push!(eax);
                call!(0x314d70);
                pc = 0x314dfd; continue;
            }
            0x314dfd => {
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x314dfe; continue;
            }
            0x314dfe => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00314E00: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f314e00<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x314e00;
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
        { crate::pose::cover(0x314e00, previous, pc); previous = pc; }
        match pc {
            0x314e00 => {
                call!(0x317cd0);
                pc = 0x314e05; continue;
            }
            0x314e05 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x314e25; continue; }
                pc = 0x314e09; continue;
            }
            0x314e09 => {
                push!(ebx);
                push!(esi);
                push!(edi);
                edi = 0;
                ebx = 0;
                esi = 0;
                call!(0x314b70);
                pc = 0x314e17; continue;
            }
            0x314e17 => {
                push!(0x7);
                call!(0x3356f0);
                pc = 0x314e1e; continue;
            }
            0x314e1e => {
                esp = esp.wrapping_add(0x4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x314e25 => {
                push!(0x7);
                call!(0x335840);
                pc = 0x314e2c; continue;
            }
            0x314e2c => {
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x003157A0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3157a0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3157a0;
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
        { crate::pose::cover(0x3157a0, previous, pc); previous = pc; }
        match pc {
            0x3157a0 => {
                eax = m.read32(0x7844a8);
                m.write8(eax.wrapping_add(0x11854), (0) as u8);
                call!(0x314c20);
                pc = 0x3157b1; continue;
            }
            0x3157b1 => {
                push!(0x7);
                call!(0x335840);
                pc = 0x3157b8; continue;
            }
            0x3157b8 => {
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x003158C0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3158c0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3158c0;
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
        { crate::pose::cover(0x3158c0, previous, pc); previous = pc; }
        match pc {
            0x3158c0 => {
                eax = m.read32(0x7ba9e0);
                eax = ((eax as i32).wrapping_mul((0x1e0) as i32)) as u32;
                ecx = m.read32(eax.wrapping_add(0x7ba26c));
                push!(ecx);
                call!(0x77470);
                pc = 0x3158d7; continue;
            }
            0x3158d7 => {
                ecx = m.read32(0x783b94);
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3158f7; continue; }
                pc = 0x3158e4; continue;
            }
            0x3158e4 => {
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x315901; continue; }
                pc = 0x3158e8; continue;
            }
            0x3158e8 => {
                fa = m.read32(eax.wrapping_add(0x1b0));
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3158f7; continue; }
                pc = 0x3158f1; continue;
            }
            0x3158f1 => {
                eax = 0x2;
                break 'run;
            }
            0x3158f7 => {
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x315901; continue; }
                pc = 0x3158fb; continue;
            }
            0x3158fb => {
                eax = 0xfffffffeu32;
                break 'run;
            }
            0x315901 => {
                edx = m.read32(0x7ba9e0);
                edx = ((edx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                eax = m.read32(edx.wrapping_add(0x7ba26c));
                push!(eax);
                call!(0x77470);
                pc = 0x315919; continue;
            }
            0x315919 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x31592a; continue; }
                pc = 0x315927; continue;
            }
            0x315927 => {
                eax = 0;
                break 'run;
            }
            0x31592a => {
                ecx = m.read32(0x7ba9e0);
                ecx = ((ecx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                edx = m.read32(ecx.wrapping_add(0x7ba26c));
                push!(edx);
                call!(0x77470);
                pc = 0x315942; continue;
            }
            0x315942 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                eax = (eax & 0xffff_ff00) | (((((((((eax) as u8)) as i32) & 0x20i32)) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                cf = (((((((((eax) as u8))) as i32) != 0i32))) as i32);
                eax = (eax & 0xffff_ff00) | (((((((((eax) as u8)) as i32).wrapping_neg())) as u32)) & 0xff);
                eax = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                eax = (eax & 0x2);
                eax = eax.wrapping_sub(1);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00315960: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f315960<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x315960;
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
        { crate::pose::cover(0x315960, previous, pc); previous = pc; }
        match pc {
            0x315960 => {
                call!(0x3158c0);
                pc = 0x315965; continue;
            }
            0x315965 => {
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                if (fa > fb) { pc = 0x315992; continue; }
                pc = 0x31596b; continue;
            }
            0x31596b => {
                jt = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x3159a4));
                if (jt == 0x315972u32) { pc = 0x315972; continue; }
                if (jt == 0x315977u32) { pc = 0x315977; continue; }
                if (jt == 0x315993u32) { pc = 0x315993; continue; }
                if (jt == 0x31599eu32) { pc = 0x31599e; continue; }
                tail_to!(jt); break 'run;  // indirect tail jump, target outside the switch
            }
            0x315972 => {
                tail_to!(0x314f30); break 'run;  // direct tail jump
            }
            0x315977 => {
                eax = m.read32(0x7844a8);
                m.write8(eax.wrapping_add(0x11854), (0) as u8);
                call!(0x314c20);
                pc = 0x315988; continue;
            }
            0x315988 => {
                push!(0x7);
                call!(0x335840);
                pc = 0x31598f; continue;
            }
            0x31598f => {
                esp = esp.wrapping_add(0x4);
                pc = 0x315992; continue;
            }
            0x315992 => {
                break 'run;
            }
            0x315993 => {
                m.write32(0x783b94, 0);
                break 'run;
            }
            0x31599e => {
                tail_to!(0x314ee0); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x003159C0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3159c0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3159c0;
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
        { crate::pose::cover(0x3159c0, previous, pc); previous = pc; }
        match pc {
            0x3159c0 => {
                call!(0x314c90);
                pc = 0x3159c5; continue;
            }
            0x3159c5 => {
                tail_to!(0x315960); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00315D10: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f315d10<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x315d10;
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
        { crate::pose::cover(0x315d10, previous, pc); previous = pc; }
        match pc {
            0x315d10 => {
                eax = 0x52ddf0;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}
}
pub use unformatted::*;
