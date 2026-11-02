use super::*;
#[rustfmt::skip] mod unformatted { use super::*;

/// 0x00304C30: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f304c30<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x304c30;
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
        { crate::pose::cover(0x304c30, previous, pc); previous = pc; }
        match pc {
            0x304c30 => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x2f1c));
                esp = esp.wrapping_sub(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                push!(ebx);
                push!(ebp);
                push!(esi);
                push!(edi);
                if (fa == fb) { pc = 0x304c66; continue; }
                pc = 0x304c47; continue;
            }
            0x304c47 => {
                eax = m.read32(eax);
                eax = ((eax as i32).wrapping_mul((0xc4) as i32)) as u32;
                fa = ((((m.read16(eax.wrapping_add(0x524f5a)))) as u32) & 0xffffu32);
                fb = ((((0x21i32)) as u32) & 0xffffu32);
                if (fa != fb) { pc = 0x304c66; continue; }
                pc = 0x304c59; continue;
            }
            0x304c59 => {
                fa = m.read32(ecx.wrapping_add(0x254));
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3051e3; continue; }
                pc = 0x304c66; continue;
            }
            0x304c66 => {
                ecx = m.read32(0x7ba9e0);
                ecx = ((ecx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                edx = m.read32(ecx.wrapping_add(0x7ba26c));
                push!(edx);
                call!(0x77470);
                pc = 0x304c7e; continue;
            }
            0x304c7e => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                eax = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x304e53; continue; }
                pc = 0x304c95; continue;
            }
            0x304c95 => {
                edx = m.read32(eax.wrapping_add(0x25c));
                ecx = 0x2000;
                fa = ecx;
                fb = edx;
                if ((fa & fb) != 0) { pc = 0x304e53; continue; }
                pc = 0x304ca8; continue;
            }
            0x304ca8 => {
                edx = m.read32(eax.wrapping_add(0x2f70));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x304e53; continue; }
                pc = 0x304cb6; continue;
            }
            0x304cb6 => {
                m.write32(eax.wrapping_add(0x25c), (m.read32(eax.wrapping_add(0x25c)) | ecx));
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f34));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                edi = eax.wrapping_add(0x2f34);
                if (fas < fbs) { pc = 0x304d7d; continue; }
                pc = 0x304cd5; continue;
            }
            0x304cd5 => {
                fa = ((((m.read8(eax.wrapping_add(0x25c)))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x304d75; continue; }
                pc = 0x304ce2; continue;
            }
            0x304ce2 => {
                eax = esp.wrapping_add(0x10);
                push!(eax);
                call!(0x331ab0);
                pc = 0x304cec; continue;
            }
            0x304cec => {
                edx = m.read32(esp.wrapping_add(0x14));
                eax = (eax & 0xffff_0000) | ((((m.read16(esp.wrapping_add(0x18))) as u32)) & 0xffff);
                push!(ecx);
                ecx = esp;
                m.write32(ecx, edx);
                write16(m, ecx.wrapping_add(0x4), (((((eax) as u16)) as u32)) as u16);
                ecx = m.read32(0x7844a8);
                edx = m.read32(ecx.wrapping_add(0x2f18));
                push!(edx);
                call!(0x334770);
                pc = 0x304d10; continue;
            }
            0x304d10 => {
                esp = esp.wrapping_add(0xc);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x304e3e; continue; }
                pc = 0x304d1b; continue;
            }
            0x304d1b => {
                push!(0x157a);
                call!(0x2f79e0);
                pc = 0x304d25; continue;
            }
            0x304d25 => {
                eax = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x4);
                push!(0);
                ecx = eax.wrapping_add(0x2f34);
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x18));
                esp = esp.wrapping_sub(0x8);
                edx = esp;
                m.write32(edx, ecx);
                ecx = (ecx & 0xffff_0000) | ((((m.read16(esp.wrapping_add(0x24))) as u32)) & 0xffff);
                write16(m, edx.wrapping_add(0x4), (((((ecx) as u16)) as u32)) as u16);
                edx = m.read32(eax.wrapping_add(0x2f18));
                push!(edx);
                call!(0x303460);
                pc = 0x304d56; continue;
            }
            0x304d56 => {
                eax = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x14);
                m.write32(eax.wrapping_add(0x2f34), 0xffffffffu32);
                call!(0x73c80);
                pc = 0x304d6d; continue;
            }
            0x304d6d => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            0x304d75 => {
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x304e0d; continue; }
                pc = 0x304d7d; continue;
            }
            0x304d7d => {
                esi = m.read32(eax.wrapping_add(0x2f1c));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x304e0d; continue; }
                pc = 0x304d8b; continue;
            }
            0x304d8b => {
                ecx = 0xf;
                { let step = if entry.eflags & 0x400 == 0 { 4u32 } else { 0u32.wrapping_sub(4) }; for i in 0..ecx { let value=m.read32(esi.wrapping_add(i.wrapping_mul(step)));m.write32(edi.wrapping_add(i.wrapping_mul(step)),value); } esi=esi.wrapping_add(ecx.wrapping_mul(step));edi=edi.wrapping_add(ecx.wrapping_mul(step));ecx=0; }
                ecx = m.read32(0x7844a8);
                eax = (eax & 0xffff_ff00) | ((((m.read8(ecx.wrapping_add(0x238))) as u32)) & 0xff);
                ebx = 0x20;
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((((eax) as u8))) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x304dc4; continue; }
                pc = 0x304da7; continue;
            }
            0x304da7 => {
                push!(0x1);
                call!(0x3370a0);
                pc = 0x304dae; continue;
            }
            0x304dae => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x238));
                esp = esp.wrapping_add(0x4);
                ecx = (ecx | ebx);
                m.write32(eax.wrapping_add(0x238), ecx);
                pc = 0x304dc4; continue;
            }
            0x304dc4 => {
                push!(0x1578);
                call!(0x2f79e0);
                pc = 0x304dce; continue;
            }
            0x304dce => {
                eax = m.read32(0x7844a8);
                edx = m.read32(eax.wrapping_add(0x2f1c));
                eax = m.read32(eax.wrapping_add(0x2f18));
                push!(edx);
                push!(eax);
                call!(0x3080b0);
                pc = 0x304de6; continue;
            }
            0x304de6 => {
                ecx = m.read32(0x7844a8);
                push!(0x5);
                m.write32(ecx.wrapping_add(0x2f1c), 0);
                call!(0x2fb930);
                pc = 0x304dfd; continue;
            }
            0x304dfd => {
                esp = esp.wrapping_add(0x10);
                call!(0x73c80);
                pc = 0x304e05; continue;
            }
            0x304e05 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            0x304e0d => {
                edx = m.read32(eax.wrapping_add(0x2f18));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x304e2a; continue; }
                pc = 0x304e17; continue;
            }
            0x304e17 => {
                edx = m.read32(eax.wrapping_add(0x2f1c));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x304e3e; continue; }
                pc = 0x304e21; continue;
            }
            0x304e21 => {
                fa = ((((m.read8(eax.wrapping_add(0x25c)))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x304e3e; continue; }
                pc = 0x304e2a; continue;
            }
            0x304e2a => {
                fa = ecx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3051e3; continue; }
                pc = 0x304e33; continue;
            }
            0x304e33 => {
                m.write32(edi, 0xffffffffu32);
                call!(0x73c80);
                pc = 0x304e3e; continue;
            }
            0x304e3e => {
                push!(0x1576);
                call!(0x2f79e0);
                pc = 0x304e48; continue;
            }
            0x304e48 => {
                esp = esp.wrapping_add(0x4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            0x304e53 => {
                ecx = m.read32(esp.wrapping_add(0x1c));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x304ea9; continue; }
                pc = 0x304e5b; continue;
            }
            0x304e5b => {
                ecx = m.read32(eax.wrapping_add(0x2f18));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x304e80; continue; }
                pc = 0x304e65; continue;
            }
            0x304e65 => {
                ecx = m.read32(eax.wrapping_add(0x2f1c));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3051e3; continue; }
                pc = 0x304e73; continue;
            }
            0x304e73 => {
                fa = ((((m.read8(eax.wrapping_add(0x25c)))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x3051e3; continue; }
                pc = 0x304e80; continue;
            }
            0x304e80 => {
                fa = m.read32(eax.wrapping_add(0x2f34));
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3051e3; continue; }
                pc = 0x304e8d; continue;
            }
            0x304e8d => {
                m.write32(eax.wrapping_add(0x2f34), 0xffffffffu32);
                call!(0x2fb020);
                pc = 0x304e9c; continue;
            }
            0x304e9c => {
                call!(0x73c80);
                pc = 0x304ea1; continue;
            }
            0x304ea1 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            0x304ea9 => {
                edx = m.read32(0x7ba9e0);
                edx = ((edx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                eax = m.read32(edx.wrapping_add(0x7ba26c));
                push!(eax);
                call!(0x77470);
                pc = 0x304ec1; continue;
            }
            0x304ec1 => {
                ecx = m.read32(eax.wrapping_add(0xc4));
                eax = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x4);
                fa = ecx;
                fb = ((((0x800000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) { pc = 0x304f44; continue; }
                pc = 0x304ed7; continue;
            }
            0x304ed7 => {
                ecx = m.read32(eax.wrapping_add(0x2f34));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x304f44; continue; }
                pc = 0x304ee1; continue;
            }
            0x304ee1 => {
                ecx = m.read32(eax.wrapping_add(0x2f38));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x304f15; continue; }
                pc = 0x304eeb; continue;
            }
            0x304eeb => {
                call!(0x2ffe10);
                pc = 0x304ef0; continue;
            }
            0x304ef0 => {
                ecx = m.read32(0x7844a8);
                edi = eax;
                eax = m.read32(ecx.wrapping_add(0x2f34));
                push!(0);
                call!(0x2ffd50);
                pc = 0x304f05; continue;
            }
            0x304f05 => {
                esp = esp.wrapping_add(0x4);
                call!(0x73c80);
                pc = 0x304f0d; continue;
            }
            0x304f0d => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            0x304f15 => {
                push!(0x1577);
                call!(0x2f79e0);
                pc = 0x304f1f; continue;
            }
            0x304f1f => {
                edx = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x4);
                m.write32(edx.wrapping_add(0x2f34), 0xffffffffu32);
                call!(0x73c80);
                pc = 0x304f37; continue;
            }
            0x304f37 => {
                call!(0x73c80);
                pc = 0x304f3c; continue;
            }
            0x304f3c => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            0x304f44 => {
                ecx = m.read32(eax.wrapping_add(0x2f70));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x304fd5; continue; }
                pc = 0x304f52; continue;
            }
            0x304f52 => {
                eax = m.read32(0x7ba9e0);
                eax = ((eax as i32).wrapping_mul((0x1e0) as i32)) as u32;
                ecx = m.read32(eax.wrapping_add(0x7ba26c));
                push!(ecx);
                call!(0x77470);
                pc = 0x304f69; continue;
            }
            0x304f69 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (((((((((((((fa)) as u8)) as i32).wrapping_sub((((((fb)) as u8)) as i32)))) as u8)) as i32).wrapping_shr((0x7 & 31))) == 0i32)) { pc = 0x304fd5; continue; }
                pc = 0x304f76; continue;
            }
            0x304f76 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f34));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x304fd5; continue; }
                pc = 0x304f85; continue;
            }
            0x304f85 => {
                eax = m.read32(eax.wrapping_add(0x2f1c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x304fd5; continue; }
                pc = 0x304f8f; continue;
            }
            0x304f8f => {
                ecx = m.read32(eax.wrapping_add(0x4));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x304fb4; continue; }
                pc = 0x304f96; continue;
            }
            0x304f96 => {
                call!(0x2ffe10);
                pc = 0x304f9b; continue;
            }
            0x304f9b => {
                edx = m.read32(0x7844a8);
                ecx = m.read32(edx.wrapping_add(0x2f1c));
                edi = eax;
                eax = m.read32(ecx);
                push!(0);
                call!(0x2ffd50);
                pc = 0x304fb2; continue;
            }
            0x304fb2 => {
                pc = 0x304fbb; continue;
            }
            0x304fb4 => {
                push!(0);
                call!(0x2fb5e0);
                pc = 0x304fbb; continue;
            }
            0x304fbb => {
                esp = esp.wrapping_add(0x4);
                push!(0x5);
                call!(0x2fb930);
                pc = 0x304fc5; continue;
            }
            0x304fc5 => {
                esp = esp.wrapping_add(0x4);
                call!(0x73c80);
                pc = 0x304fcd; continue;
            }
            0x304fcd => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            0x304fd5 => {
                edx = m.read32(0x7ba9e0);
                edx = ((edx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                eax = m.read32(edx.wrapping_add(0x7ba26c));
                push!(eax);
                call!(0x77470);
                pc = 0x304fed; continue;
            }
            0x304fed => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                ebx = (ebx & 0xffff_ff00) | ((0x40) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((((ecx) as u8))) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x30506d; continue; }
                pc = 0x304ffc; continue;
            }
            0x304ffc => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f70));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x30506d; continue; }
                pc = 0x30500b; continue;
            }
            0x30500b => {
                ecx = m.read32(eax.wrapping_add(0x2f34));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x30506d; continue; }
                pc = 0x305015; continue;
            }
            0x305015 => {
                eax = m.read32(eax.wrapping_add(0x2f1c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30506d; continue; }
                pc = 0x30501f; continue;
            }
            0x30501f => {
                ebp = m.read32(eax);
                ebp = ((ebp as i32).wrapping_mul((0xc4) as i32)) as u32;
                push!(0x1578);
                ebp = ebp.wrapping_add(0x524f58);
                call!(0x2f79e0);
                pc = 0x305037; continue;
            }
            0x305037 => {
                eax = m.read32(0x7844a8);
                esi = m.read32(eax.wrapping_add(0x2f1c));
                edi = eax.wrapping_add(0x2f34);
                ecx = 0xf;
                { let step = if entry.eflags & 0x400 == 0 { 4u32 } else { 0u32.wrapping_sub(4) }; for i in 0..ecx { let value=m.read32(esi.wrapping_add(i.wrapping_mul(step)));m.write32(edi.wrapping_add(i.wrapping_mul(step)),value); } esi=esi.wrapping_add(ecx.wrapping_mul(step));edi=edi.wrapping_add(ecx.wrapping_mul(step));ecx=0; }
                eax = m.read32(ebp.wrapping_add(0xb8));
                esp = esp.wrapping_add(0x4);
                fa = (((((((eax) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x30506d; continue; }
                pc = 0x30505d; continue;
            }
            0x30505d => {
                ecx = m.read32(0x7844a8);
                m.write32(ecx.wrapping_add(0x2f38), 0xffffffffu32);
                pc = 0x30506d; continue;
            }
            0x30506d => {
                edx = m.read32(0x7ba9e0);
                edx = ((edx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                eax = m.read32(edx.wrapping_add(0x7ba26c));
                push!(eax);
                call!(0x77470);
                pc = 0x305085; continue;
            }
            0x305085 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((((ecx) as u8))) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x3050ec; continue; }
                pc = 0x305092; continue;
            }
            0x305092 => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x2f34));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x3050ec; continue; }
                pc = 0x3050a2; continue;
            }
            0x3050a2 => {
                fa = ((((m.read8(ecx.wrapping_add(0x25c)))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x3050ec; continue; }
                pc = 0x3050ab; continue;
            }
            0x3050ab => {
                eax = ((eax as i32).wrapping_mul((0xc4) as i32)) as u32;
                fa = ((((m.read8(eax.wrapping_add(0x525010)))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x3050ec; continue; }
                pc = 0x3050ba; continue;
            }
            0x3050ba => {
                push!(0x1579);
                call!(0x2f79e0);
                pc = 0x3050c4; continue;
            }
            0x3050c4 => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x2f3c));
                esp = esp.wrapping_add(0x4);
                eax = eax.wrapping_sub(1);
                fa = eax;
                fb = ((((0x7i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x3050dd; continue; }
                pc = 0x3050d9; continue;
            }
            0x3050d9 => {
                eax = 0;
                pc = 0x3050e6; continue;
            }
            0x3050dd => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x3050e6; continue; }
                pc = 0x3050e1; continue;
            }
            0x3050e1 => {
                eax = 0x7;
                pc = 0x3050e6; continue;
            }
            0x3050e6 => {
                m.write32(ecx.wrapping_add(0x2f3c), eax);
                pc = 0x3050ec; continue;
            }
            0x3050ec => {
                ecx = m.read32(0x7ba9e0);
                ecx = ((ecx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                edx = m.read32(ecx.wrapping_add(0x7ba26c));
                push!(edx);
                call!(0x77470);
                pc = 0x305104; continue;
            }
            0x305104 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                ebx = 0x20;
                esp = esp.wrapping_add(0x4);
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((((ecx) as u8))) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x305170; continue; }
                pc = 0x305116; continue;
            }
            0x305116 => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x2f34));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x305170; continue; }
                pc = 0x305126; continue;
            }
            0x305126 => {
                fa = ((((m.read8(ecx.wrapping_add(0x25c)))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x305170; continue; }
                pc = 0x30512f; continue;
            }
            0x30512f => {
                eax = ((eax as i32).wrapping_mul((0xc4) as i32)) as u32;
                fa = ((((m.read8(eax.wrapping_add(0x525010)))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x305170; continue; }
                pc = 0x30513e; continue;
            }
            0x30513e => {
                push!(0x1579);
                call!(0x2f79e0);
                pc = 0x305148; continue;
            }
            0x305148 => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x2f3c));
                esp = esp.wrapping_add(0x4);
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ((((0x7i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x305161; continue; }
                pc = 0x30515d; continue;
            }
            0x30515d => {
                eax = 0;
                pc = 0x30516a; continue;
            }
            0x305161 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x30516a; continue; }
                pc = 0x305165; continue;
            }
            0x305165 => {
                eax = 0x7;
                pc = 0x30516a; continue;
            }
            0x30516a => {
                m.write32(ecx.wrapping_add(0x2f3c), eax);
                pc = 0x305170; continue;
            }
            0x305170 => {
                eax = m.read32(0x7ba9e0);
                eax = ((eax as i32).wrapping_mul((0x1e0) as i32)) as u32;
                ecx = m.read32(eax.wrapping_add(0x7ba26c));
                push!(ecx);
                call!(0x77470);
                pc = 0x305187; continue;
            }
            0x305187 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((((ecx) as u8))) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x3051e3; continue; }
                pc = 0x305194; continue;
            }
            0x305194 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f34));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x3051e3; continue; }
                pc = 0x3051a3; continue;
            }
            0x3051a3 => {
                eax = m.read32(eax.wrapping_add(0x2f1c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3051e3; continue; }
                pc = 0x3051ad; continue;
            }
            0x3051ad => {
                edx = m.read32(eax);
                push!(edx);
                call!(0x2f7770);
                pc = 0x3051b5; continue;
            }
            0x3051b5 => {
                ecx = m.read32(eax.wrapping_add(0x8));
                esp = esp.wrapping_add(0x4);
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3051e3; continue; }
                pc = 0x3051bf; continue;
            }
            0x3051bf => {
                eax = m.read32(0x7844a8);
                eax = m.read32(eax.wrapping_add(0x2f1c));
                ecx = m.read32(eax);
                fa = ecx;
                fb = ((((0x47i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3051e3; continue; }
                pc = 0x3051d1; continue;
            }
            0x3051d1 => {
                push!(ecx);
                call!(0x2f7770);
                pc = 0x3051d7; continue;
            }
            0x3051d7 => {
                eax = m.read32(eax.wrapping_add(0x8));
                push!(eax);
                call!(0x2fb230);
                pc = 0x3051e0; continue;
            }
            0x3051e0 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x3051e3; continue;
            }
            0x3051e3 => {
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
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}
}
pub use unformatted::*;
