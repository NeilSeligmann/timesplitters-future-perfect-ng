use super::*;
#[rustfmt::skip] mod unformatted { use super::*;
/// 0x003163D0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3163d0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3163d0;
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
        { crate::pose::cover(0x3163d0, previous, pc); previous = pc; }
        match pc {
            0x3163d0 => {
                push!(ecx);
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0xc));
                push!(ebp);
                push!(esi);
                esi = m.read32(0x78400c);
                ebp = 0;
                fa = ebx;
                fb = esi;
                push!(edi);
                if (fa != fb) { pc = 0x3163e8; continue; }
                pc = 0x3163e5; continue;
            }
            0x3163e5 => {
                esi = m.read32(esi.wrapping_add(0x40));
                pc = 0x3163e8; continue;
            }
            0x3163e8 => {
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3164e3; continue; }
                pc = 0x3163f0; continue;
            }
            0x3163f0 => {
                ecx = m.read32(0x7844a8);
                pc = 0x3163f6; continue;
            }
            0x3163f6 => {
                eax = m.read32(esi.wrapping_add(0xc));
                edi = m.read32(esi.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x2c));
                eax = m.read32(edi.wrapping_add(0x44));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x10), edi);
                m.write32(esp.wrapping_add(0x18), 0);
                if (fas <= fbs) { pc = 0x3164d3; continue; }
                pc = 0x316414; continue;
            }
            0x316414 => {
                edi = edi.wrapping_add(0x48);
                pc = 0x316420; continue;
            }
            0x316420 => {
                edx = ((m.read8(edi)) as u8 as u32);
                fa = edx;
                fb = m.read32(esp.wrapping_add(0x1c));
                if (fa != fb) { pc = 0x3164ba; continue; }
                pc = 0x31642d; continue;
            }
            0x31642d => {
                eax = m.read32(ecx.wrapping_add(0x2f7c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31644b; continue; }
                pc = 0x316437; continue;
            }
            0x316437 => {
                fa = ebx;
                fb = eax;
                if (fa == fb) { pc = 0x316444; continue; }
                pc = 0x31643b; continue;
            }
            0x31643b => {
                eax = m.read32(eax.wrapping_add(0x40));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x316437; continue; }
                pc = 0x316442; continue;
            }
            0x316442 => {
                pc = 0x31644b; continue;
            }
            0x316444 => {
                m.write32(ecx.wrapping_add(0x25c), (m.read32(ecx.wrapping_add(0x25c)) | 0x10));
                pc = 0x31644b; continue;
            }
            0x31644b => {
                eax = ((m.read8(edi.wrapping_add(0x4))) as u8 as u32);
                edx = ((m.read8(edi.wrapping_add(0x3))) as u8 as u32);
                ecx = ((m.read8(edi)) as u8 as u32);
                push!(eax);
                push!(ecx);
                ecx = (((((f32::from_bits(m.read32(esi.wrapping_add(0x4))) as f64)) as i32)) as u32);
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x8)))).to_bits();
                edx = (((((f32::from_bits(m.read32(esi)) as f64)) as i32)) as u32);
                eax = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(eax);
                push!(ecx);
                push!(edx);
                push!(ebx);
                call!(0x2f8780);
                pc = 0x316477; continue;
            }
            0x316477 => {
                ecx = m.read32(0x7844a8);
                xmm0.lanes[0] = (((m.read32(ecx.wrapping_add(0x264))) as i32) as f32).to_bits();
                xmm1 = Xmm::scalar(m.read32(esi.wrapping_add(0x8)));
                ebp = eax;
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                eax = sse::cvttss2si(f32::from_bits(xmm1.lanes[0]));
                edx = if ((((eax) as i32) < 0i32)) { 0xffffffffu32 } else { 0 };
                eax = (eax ^ edx);
                eax = eax.wrapping_sub(edx);
                esp = esp.wrapping_add(0x18);
                fa = eax;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x3164a4; continue; }
                pc = 0x3164a1; continue;
            }
            0x3164a1 => {
                ebp = (ebp | 0xffffffffu32);
                pc = 0x3164a4; continue;
            }
            0x3164a4 => {
                fa = ((((m.read8(ecx.wrapping_add(0x25c)))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x3164ba; continue; }
                pc = 0x3164ad; continue;
            }
            0x3164ad => {
                m.write32(ecx.wrapping_add(0x25c), (m.read32(ecx.wrapping_add(0x25c)) & 0xffffffefu32));
                ecx = m.read32(0x7844a8);
                pc = 0x3164ba; continue;
            }
            0x3164ba => {
                eax = m.read32(esp.wrapping_add(0x18));
                edx = m.read32(esp.wrapping_add(0x10));
                eax = eax.wrapping_add(1);
                edi = edi.wrapping_add(0x6);
                fa = eax;
                fb = m.read32(edx.wrapping_add(0x44));
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x18), eax);
                if (fas < fbs) { pc = 0x316420; continue; }
                pc = 0x3164d3; continue;
            }
            0x3164d3 => {
                fa = ebp;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3164eb; continue; }
                pc = 0x3164d8; continue;
            }
            0x3164d8 => {
                esi = m.read32(esi.wrapping_add(0x40));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3163f6; continue; }
                pc = 0x3164e3; continue;
            }
            0x3164e3 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x3164eb => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = esi;
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
/// 0x00316920: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f316920<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x316920;
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
        { crate::pose::cover(0x316920, previous, pc); previous = pc; }
        match pc {
            0x316920 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                push!(esi);
                call!(0x71de0);
                pc = 0x31692b; continue;
            }
            0x31692b => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x316b20; continue; }
                pc = 0x316936; continue;
            }
            0x316936 => {
                esi = m.read32(esi.wrapping_add(0x18c));
                fa = esi;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x316b20; continue; }
                pc = 0x316945; continue;
            }
            0x316945 => {
                eax = m.read32(0x778d34);
                push!(0x2);
                push!(esi);
                m.write32(eax, 0x1);
                call!(0x32d3a0);
                pc = 0x316958; continue;
            }
            0x316958 => {
                ecx = m.read32(0x778d34);
                m.write32(ecx.wrapping_add(0x38), (fpu.st(0) as f32).to_bits()); fpu.pop();
                edx = m.read32(0x778d34);
                m.write32(edx.wrapping_add(0x80), 0x4);
                eax = m.read32(0x778d34);
                ecx = m.read32(eax.wrapping_add(0x80));
                ecx = (ecx | 0x50);
                push!(0);
                m.write32(eax.wrapping_add(0x80), ecx);
                call!(0x32f360);
                pc = 0x31698c; continue;
            }
            0x31698c => {
                eax = m.read32(0x778d34);
                ecx = m.read32(eax.wrapping_add(0x90));
                esp = esp.wrapping_add(0xc);
                fa = esi;
                fb = ecx;
                if (fa == fb) { pc = 0x316b1a; continue; }
                pc = 0x3169a2; continue;
            }
            0x3169a2 => {
                push!(edi);
                push!(0x157e);
                call!(0x2f79e0);
                pc = 0x3169ad; continue;
            }
            0x3169ad => {
                ecx = m.read32(0x778d34);
                m.write32(ecx.wrapping_add(0x90), esi);
                xmm0 = Xmm::scalar(m.read32(0x7ba188));
                xmm2 = Xmm::scalar(m.read32(0x7ba184));
                ecx = m.read32(0x4e8cf0);
                xmm1 = xmm0;
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x49f7b0))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x47d1ec))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d10))).to_bits();
                edx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475cd4))).to_bits();
                push!(edx);
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475db4))).to_bits();
                push!(0xb4);
                eax = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(eax);
                eax = ecx;
                edx = if ((((eax) as i32) < 0i32)) { 0xffffffffu32 } else { 0 };
                eax = eax.wrapping_sub(edx);
                eax = (((((eax as i32)).wrapping_shr((((((((0x1i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                eax = eax.wrapping_sub(1);
                edx = if ((((eax) as i32) < 0i32)) { 0xffffffffu32 } else { 0 };
                eax = eax.wrapping_sub(edx);
                eax = (((((eax as i32)).wrapping_shr((((((((0x1i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                ecx = ecx.wrapping_sub(eax);
                ecx = ecx.wrapping_add(0x3);
                push!(ecx);
                call!(0x32d1a0);
                pc = 0x316a31; continue;
            }
            0x316a31 => {
                ecx = m.read32(0x4e8cf0);
                xmm0 = Xmm::scalar(m.read32(0x7ba188));
                xmm2 = Xmm::scalar(m.read32(0x7ba184));
                eax = ecx;
                edx = if ((((eax) as i32) < 0i32)) { 0xffffffffu32 } else { 0 };
                eax = eax.wrapping_sub(edx);
                eax = (((((eax as i32)).wrapping_shr((((((((0x1i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                edi = eax.wrapping_add(((0x1i32.wrapping_neg()) as u32));
                eax = m.read32(0x4e8cec);
                edx = if ((((eax) as i32) < 0i32)) { 0xffffffffu32 } else { 0 };
                xmm1 = xmm0;
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x49f7b0))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x47d1ec))).to_bits();
                edx = (edx & 0x7);
                eax = eax.wrapping_add(edx);
                eax = (((((eax as i32)).wrapping_shr((((((((0x3i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                push!(eax);
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d10))).to_bits();
                edx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475cd4))).to_bits();
                push!(edx);
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475db4))).to_bits();
                push!(edi);
                eax = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(eax);
                eax = edi;
                edx = if ((((eax) as i32) < 0i32)) { 0xffffffffu32 } else { 0 };
                eax = eax.wrapping_sub(edx);
                eax = (((((eax as i32)).wrapping_shr((((((((0x1i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                ecx = ecx.wrapping_sub(eax);
                ecx = ecx.wrapping_add(0x3);
                push!(ecx);
                call!(0x32d260);
                pc = 0x316ab8; continue;
            }
            0x316ab8 => {
                push!(esi);
                call!(0x32cfe0);
                pc = 0x316abe; continue;
            }
            0x316abe => {
                eax = m.read32(0x7844a8);
                ecx = esi.wrapping_add(esi.wrapping_mul(0x4));
                ecx = ecx.wrapping_shl((((((((0x6i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = ((m.read8(ecx.wrapping_add(0x529520))) as u8 as u32);
                ecx = m.read32(eax.wrapping_add(edx.wrapping_mul(0x4)).wrapping_add(0x4064));
                edx = m.read32(0x778d34);
                m.write32(edx.wrapping_add(0x84), ecx);
                push!(esi);
                call!(0x311c00);
                pc = 0x316ae9; continue;
            }
            0x316ae9 => {
                ecx = m.read32(0x778d34);
                edx = m.read32(ecx.wrapping_add(0x84));
                ecx = m.read32(edx.wrapping_add(0x4));
                m.write32(ecx.wrapping_add(0x30), eax);
                edx = m.read32(0x778d34);
                eax = m.read32(edx.wrapping_add(0x84));
                eax = m.read32(eax.wrapping_add(0x4));
                ecx = m.read32(eax.wrapping_add(0x30));
                esp = esp.wrapping_add(0x30);
                ecx = (ecx | 0x2000000);
                m.write32(eax.wrapping_add(0x30), ecx);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x316b1a; continue;
            }
            0x316b1a => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                tail_to!(0x32d320); break 'run;  // direct tail jump
            }
            0x316b20 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00316D10: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f316d10<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x316d10;
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
        { crate::pose::cover(0x316d10, previous, pc); previous = pc; }
        match pc {
            0x316d10 => {
                ecx = m.read32(0x78400c);
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x316d46; continue; }
                pc = 0x316d1a; continue;
            }
            0x316d1a => {
                pc = 0x316d20; continue;
            }
            0x316d20 => {
                eax = m.read32(ecx.wrapping_add(0x3c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x316d3f; continue; }
                pc = 0x316d27; continue;
            }
            0x316d27 => {
                edx = m.read32(esi);
                pc = 0x316d30; continue;
            }
            0x316d30 => {
                fa = m.read32(eax);
                fb = edx;
                if (fa != fb) { pc = 0x316d38; continue; }
                pc = 0x316d34; continue;
            }
            0x316d34 => {
                fa = eax;
                fb = esi;
                if (fa != fb) { pc = 0x316d48; continue; }
                pc = 0x316d38; continue;
            }
            0x316d38 => {
                eax = m.read32(eax.wrapping_add(0x14));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x316d30; continue; }
                pc = 0x316d3f; continue;
            }
            0x316d3f => {
                ecx = m.read32(ecx.wrapping_add(0x40));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x316d20; continue; }
                pc = 0x316d46; continue;
            }
            0x316d46 => {
                eax = 0;
                pc = 0x316d48; continue;
            }
            0x316d48 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00316D50: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f316d50<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x316d50;
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
        { crate::pose::cover(0x316d50, previous, pc); previous = pc; }
        match pc {
            0x316d50 => {
                edx = m.read32(eax);
                m.write32(ecx, edx);
                edx = m.read32(eax.wrapping_add(0x4));
                m.write32(ecx.wrapping_add(0x4), edx);
                eax = m.read32(eax.wrapping_add(0x8));
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x316d77; continue; }
                pc = 0x316d62; continue;
            }
            0x316d62 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x316d6f; continue; }
                pc = 0x316d66; continue;
            }
            0x316d66 => {
                eax = eax.wrapping_sub(1);
                cf = (((((eax) != 0))) as i32);
                eax = (((((eax) as i32).wrapping_neg())) as u32);
                eax = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                m.write32(ecx.wrapping_add(0x8), eax);
                break 'run;
            }
            0x316d6f => {
                m.write32(ecx.wrapping_add(0x8), 0x1);
                break 'run;
            }
            0x316d77 => {
                m.write32(ecx.wrapping_add(0x8), 0xffffffffu32);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00316D80: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f316d80<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x316d80;
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
        { crate::pose::cover(0x316d80, previous, pc); previous = pc; }
        match pc {
            0x316d80 => {
                eax = m.read32(0x7844a8);
                edx = m.read32(eax.wrapping_add(0x2f34));
                ecx = edx;
                ecx = ((ecx as i32).wrapping_mul((0xc4) as i32)) as u32;
                ecx = (((((m.read16(ecx.wrapping_add(0x524f5a)) as i16)) as i32)) as u32);
                ecx = ecx.wrapping_sub(1);
                fa = ecx;
                fb = ((((0x21i32)) as u32) & 0xffffffffu32);
                if (fa > fb) { pc = 0x316f96; continue; }
                pc = 0x316da4; continue;
            }
            0x316da4 => {
                ecx = ((m.read8(ecx.wrapping_add(0x316fbc))) as u8 as u32);
                jt = m.read32(ecx.wrapping_mul(0x4).wrapping_add(0x316f98));
                if (jt == 0x316db2u32) { pc = 0x316db2; continue; }
                if (jt == 0x316e2cu32) { pc = 0x316e2c; continue; }
                if (jt == 0x316eb0u32) { pc = 0x316eb0; continue; }
                if (jt == 0x316ebbu32) { pc = 0x316ebb; continue; }
                if (jt == 0x316f06u32) { pc = 0x316f06; continue; }
                if (jt == 0x316f11u32) { pc = 0x316f11; continue; }
                if (jt == 0x316f2cu32) { pc = 0x316f2c; continue; }
                if (jt == 0x316f47u32) { pc = 0x316f47; continue; }
                if (jt == 0x316f96u32) { pc = 0x316f96; continue; }
                tail_to!(jt); break 'run;  // indirect tail jump, target outside the switch
            }
            0x316db2 => {
                xmm0 = Xmm::scalar(m.read32(0x475d24));
                m.write32(eax.wrapping_add(0x2f4c), xmm0.lanes[0]);
                edx = m.read32(0x7844a8);
                xmm0 = Xmm::scalar(m.read32(0x475d4c));
                m.write32(edx.wrapping_add(0x2f50), xmm0.lanes[0]);
                eax = m.read32(0x7844a8);
                ecx = 0;
                m.write32(eax.wrapping_add(0x2f58), ecx);
                edx = m.read32(0x7844a8);
                m.write32(edx.wrapping_add(0x2f5c), ecx);
                eax = m.read32(0x7844a8);
                m.write32(eax.wrapping_add(0x2f54), 0x2a);
                edx = m.read32(0x7844a8);
                m.write32(edx.wrapping_add(0x2f60), 0xc);
                eax = m.read32(0x7844a8);
                m.write32(eax.wrapping_add(0x2f68), 0x1);
                edx = m.read32(0x7844a8);
                m.write8(edx.wrapping_add(0x2f6c), (((((ecx) as u8)) as u32)) as u8);
                break 'run;
            }
            0x316e2c => {
                xmm0 = Xmm::scalar(m.read32(0x475cd4));
                m.write32(eax.wrapping_add(0x2f4c), xmm0.lanes[0]);
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x2f34));
                eax = eax.wrapping_sub(0x32);
                fa = eax;
                if ((fa == 0)) { pc = 0x316ea1; continue; }
                pc = 0x316e4d; continue;
            }
            0x316e4d => {
                eax = eax.wrapping_sub(1);
                fa = eax;
                if ((fa == 0)) { pc = 0x316e87; continue; }
                pc = 0x316e50; continue;
            }
            0x316e50 => {
                eax = eax.wrapping_sub(1);
                fa = eax;
                if ((fa == 0)) { pc = 0x316e6d; continue; }
                pc = 0x316e53; continue;
            }
            0x316e53 => {
                m.write32(ecx.wrapping_add(0x2f50), 0x23);
                eax = m.read32(0x7844a8);
                m.write32(eax.wrapping_add(0x2f54), 0);
                break 'run;
            }
            0x316e6d => {
                m.write32(ecx.wrapping_add(0x2f50), 0xd);
                eax = m.read32(0x7844a8);
                m.write32(eax.wrapping_add(0x2f54), 0);
                break 'run;
            }
            0x316e87 => {
                m.write32(ecx.wrapping_add(0x2f50), 0x10);
                eax = m.read32(0x7844a8);
                m.write32(eax.wrapping_add(0x2f54), 0);
                break 'run;
            }
            0x316ea1 => {
                m.write32(ecx.wrapping_add(0x2f50), 0x2a);
                eax = m.read32(0x7844a8);
                pc = 0x316eb0; continue;
            }
            0x316eb0 => {
                m.write32(eax.wrapping_add(0x2f54), 0);
                break 'run;
            }
            0x316ebb => {
                ecx = (ecx | 0xffffffffu32);
                m.write32(eax.wrapping_add(0x2f50), ecx);
                edx = m.read32(0x7844a8);
                m.write32(edx.wrapping_add(0x2f4c), ecx);
                eax = m.read32(0x7844a8);
                push!(esi);
                push!(edi);
                m.write32(eax.wrapping_add(0x2f54), ecx);
                edi = m.read32(0x7844a8);
                esi = edi.wrapping_add(0x2f34);
                call!(0x316d10);
                pc = 0x316eee; continue;
            }
            0x316eee => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x316f94; continue; }
                pc = 0x316ef6; continue;
            }
            0x316ef6 => {
                ecx = edi.wrapping_add(0x2f4c);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = eax.wrapping_add(0x18);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                tail_to!(0x316d50); break 'run;  // direct tail jump
            }
            0x316f06 => {
                m.write32(eax.wrapping_add(0x2f4c), 0x1);
                break 'run;
            }
            0x316f11 => {
                m.write32(eax.wrapping_add(0x2f4c), 0);
                ecx = m.read32(0x7844a8);
                m.write32(ecx.wrapping_add(0x2f50), 0x19);
                break 'run;
            }
            0x316f2c => {
                m.write32(eax.wrapping_add(0x2f4c), 0);
                edx = m.read32(0x7844a8);
                m.write32(edx.wrapping_add(0x2f50), 0x3c);
                break 'run;
            }
            0x316f47 => {
                fa = edx;
                fb = ((((0x43i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x316f96; continue; }
                pc = 0x316f4c; continue;
            }
            0x316f4c => {
                ecx = 0;
                m.write32(eax.wrapping_add(0x2f4c), ecx);
                eax = m.read32(0x7844a8);
                m.write32(eax.wrapping_add(0x2f54), ecx);
                edx = m.read32(0x7844a8);
                m.write32(edx.wrapping_add(0x2f58), ecx);
                edx = m.read32(0x7844a8);
                eax = 0x3;
                m.write32(edx.wrapping_add(0x2f5c), eax);
                edx = m.read32(0x7844a8);
                m.write32(edx.wrapping_add(0x2f60), eax);
                eax = m.read32(0x7844a8);
                m.write32(eax.wrapping_add(0x2f50), ecx);
                break 'run;
            }
            0x316f94 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x316f96; continue;
            }
            0x316f96 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00316FE0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f316fe0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x316fe0;
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
        { crate::pose::cover(0x316fe0, previous, pc); previous = pc; }
        match pc {
            0x316fe0 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                eax = m.read32(esi.wrapping_add(0x18c));
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x317029; continue; }
                pc = 0x316ff0; continue;
            }
            0x316ff0 => {
                ecx = m.read32(0x7844a8);
                m.write32(ecx.wrapping_add(0x2f34), eax);
                edx = m.read32(0x7844a8);
                m.write32(edx.wrapping_add(0x2f3c), 0);
                eax = m.read32(0x7844a8);
                m.write32(eax.wrapping_add(0x2f38), 0xffffffffu32);
                call!(0x316d80);
                pc = 0x317020; continue;
            }
            0x317020 => {
                push!(esi);
                call!(0x2fb090);
                pc = 0x317026; continue;
            }
            0x317026 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x317029; continue;
            }
            0x317029 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x003170A0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3170a0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3170a0;
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
        { crate::pose::cover(0x3170a0, previous, pc); previous = pc; }
        match pc {
            0x3170a0 => {
                esp = esp.wrapping_sub(0x28);
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0x30));
                fa = ebp;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                push!(esi);
                if (fas <= fbs) { pc = 0x3176cb; continue; }
                pc = 0x3170b2; continue;
            }
            0x3170b2 => {
                fa = ebp;
                fb = ((((0x50i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x3176cb; continue; }
                pc = 0x3170bb; continue;
            }
            0x3170bb => {
                eax = m.read32(0x778d34);
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                m.write32(eax, 0x1);
                ecx = m.read32(0x778d34);
                m.write32(ecx.wrapping_add(0x90), 0xffffffffu32);
                edx = m.read32(0x778d34);
                m.write32(esp.wrapping_add(0x34), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x475d18));
                m.write32(edx.wrapping_add(0x38), xmm0.lanes[0]);
                eax = m.read32(0x778d34);
                push!(ebx);
                ebx = 0;
                m.write32(eax.wrapping_add(0x8c), ebx);
                ecx = m.read32(0x7844a8);
                edx = m.read32(ecx.wrapping_add(0x4150));
                eax = m.read32(0x778d34);
                m.write32(eax.wrapping_add(0x84), edx);
                ecx = m.read32(0x778d34);
                m.write32(ecx.wrapping_add(0x80), 0x244);
                edx = m.read32(0x7844a8);
                fa = m.read32(edx.wrapping_add(0x4150));
                fb = ebx;
                push!(edi);
                if (fa == fb) { pc = 0x317167; continue; }
                pc = 0x31713b; continue;
            }
            0x31713b => {
                push!(ebp);
                call!(0x2fe840);
                pc = 0x317141; continue;
            }
            0x317141 => {
                ecx = m.read32(0x7844a8);
                edx = m.read32(ecx.wrapping_add(0x4150));
                ecx = ((m.read16(edx.wrapping_add(0x94))) as u16 as u32);
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ecx;
                if (fa == fb) { pc = 0x317167; continue; }
                pc = 0x31715b; continue;
            }
            0x31715b => {
                edx = m.read32(0x778d34);
                m.write32(edx.wrapping_add(0x84), ebx);
                pc = 0x317167; continue;
            }
            0x317167 => {
                eax = m.read32(0x7844a8);
                eax = m.read32(eax.wrapping_add(0x4150));
                fa = eax;
                fb = ebx;
                esi = 0x7a1;
                if (fa == fb) { pc = 0x31718d; continue; }
                pc = 0x31717b; continue;
            }
            0x31717b => {
                fa = ((((m.read16(eax.wrapping_add(0x94)))) as u32) & 0xffffu32);
                fb = ((((((esi) as u16))) as u32) & 0xffffu32);
                if (fa == fb) { pc = 0x31718d; continue; }
                pc = 0x317184; continue;
            }
            0x317184 => {
                push!(ebx);
                call!(0x32f360);
                pc = 0x31718a; continue;
            }
            0x31718a => {
                esp = esp.wrapping_add(0x4);
                pc = 0x31718d; continue;
            }
            0x31718d => {
                ecx = m.read32(0x778d34);
                fa = ebp;
                fb = m.read32(ecx.wrapping_add(0x88));
                if (fa == fb) { pc = 0x31732a; continue; }
                pc = 0x31719f; continue;
            }
            0x31719f => {
                push!(0x157e);
                call!(0x2f79e0);
                pc = 0x3171a9; continue;
            }
            0x3171a9 => {
                edx = m.read32(0x778d34);
                m.write32(edx.wrapping_add(0x88), ebp);
                eax = m.read32(0x7844a8);
                eax = m.read32(eax.wrapping_add(0x4150));
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ebx;
                if (fa == fb) { pc = 0x317203; continue; }
                pc = 0x3171c7; continue;
            }
            0x3171c7 => {
                fa = ((((m.read16(eax.wrapping_add(0x94)))) as u32) & 0xffffu32);
                fb = ((((((esi) as u16))) as u32) & 0xffffu32);
                if (fa != fb) { pc = 0x3171e3; continue; }
                pc = 0x3171d0; continue;
            }
            0x3171d0 => {
                esi = m.read32(eax.wrapping_add(0x7c));
                fa = esi;
                fb = ebx;
                if (fa == fb) { pc = 0x3171e3; continue; }
                pc = 0x3171d7; continue;
            }
            0x3171d7 => {
                push!(esi);
                call!(0xff200);
                pc = 0x3171dd; continue;
            }
            0x3171dd => {
                esp = esp.wrapping_add(0x4);
                m.write32(esi.wrapping_add(0x14), ebx);
                pc = 0x3171e3; continue;
            }
            0x3171e3 => {
                ecx = m.read32(0x7844a8);
                edx = m.read32(ecx.wrapping_add(0x4150));
                push!(edx);
                call!(0xa9070);
                pc = 0x3171f5; continue;
            }
            0x3171f5 => {
                eax = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x4);
                m.write32(eax.wrapping_add(0x4150), ebx);
                pc = 0x317203; continue;
            }
            0x317203 => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x4154));
                fa = eax;
                fb = ebx;
                if (fa == fb) { pc = 0x317228; continue; }
                pc = 0x317213; continue;
            }
            0x317213 => {
                push!(eax);
                call!(0xa9070);
                pc = 0x317219; continue;
            }
            0x317219 => {
                edx = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x4);
                m.write32(edx.wrapping_add(0x4154), ebx);
                pc = 0x317228; continue;
            }
            0x317228 => {
                xmm0 = Xmm::scalar(m.read32(0x7ba188));
                xmm2 = Xmm::scalar(m.read32(0x7ba184));
                xmm1 = xmm0;
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x49f7b0))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x47d1ec))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d10))).to_bits();
                eax = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475cd4))).to_bits();
                push!(eax);
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475db4))).to_bits();
                push!(0xb4);
                ecx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(ecx);
                ecx = m.read32(0x4e8cf0);
                eax = ecx;
                edx = if ((((eax) as i32) < 0i32)) { 0xffffffffu32 } else { 0 };
                eax = eax.wrapping_sub(edx);
                eax = (((((eax as i32)).wrapping_shr((((((((0x1i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                eax = eax.wrapping_sub(1);
                edx = if ((((eax) as i32) < 0i32)) { 0xffffffffu32 } else { 0 };
                eax = eax.wrapping_sub(edx);
                eax = (((((eax as i32)).wrapping_shr((((((((0x1i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                ecx = ecx.wrapping_sub(eax);
                ecx = ecx.wrapping_add(0x3);
                push!(ecx);
                call!(0x32d1a0);
                pc = 0x3172a0; continue;
            }
            0x3172a0 => {
                ecx = m.read32(0x4e8cf0);
                xmm0 = Xmm::scalar(m.read32(0x7ba188));
                xmm2 = Xmm::scalar(m.read32(0x7ba184));
                eax = ecx;
                edx = if ((((eax) as i32) < 0i32)) { 0xffffffffu32 } else { 0 };
                eax = eax.wrapping_sub(edx);
                eax = (((((eax as i32)).wrapping_shr((((((((0x1i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                esi = eax.wrapping_add(((0x1i32.wrapping_neg()) as u32));
                eax = m.read32(0x4e8cec);
                edx = if ((((eax) as i32) < 0i32)) { 0xffffffffu32 } else { 0 };
                xmm1 = xmm0;
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x49f7b0))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x47d1ec))).to_bits();
                edx = (edx & 0x7);
                eax = eax.wrapping_add(edx);
                eax = (((((eax as i32)).wrapping_shr((((((((0x3i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                push!(eax);
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d10))).to_bits();
                edx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475cd4))).to_bits();
                push!(edx);
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475db4))).to_bits();
                push!(esi);
                eax = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(eax);
                eax = esi;
                edx = if ((((eax) as i32) < 0i32)) { 0xffffffffu32 } else { 0 };
                eax = eax.wrapping_sub(edx);
                eax = (((((eax as i32)).wrapping_shr((((((((0x1i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                ecx = ecx.wrapping_sub(eax);
                ecx = ecx.wrapping_add(0x3);
                push!(ecx);
                call!(0x32d260);
                pc = 0x317327; continue;
            }
            0x317327 => {
                esp = esp.wrapping_add(0x24);
                pc = 0x31732a; continue;
            }
            0x31732a => {
                ecx = m.read32(0x7844a8);
                fa = m.read32(ecx.wrapping_add(0x4150));
                fb = ebx;
                if (fa != fb) { pc = 0x3175b4; continue; }
                pc = 0x31733c; continue;
            }
            0x31733c => {
                push!(ebp);
                call!(0x2fe840);
                pc = 0x317342; continue;
            }
            0x317342 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3175b4; continue; }
                pc = 0x31734e; continue;
            }
            0x31734e => {
                xmm0 = Xmm::default();
                push!(ebp);
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x34), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x38), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x28), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                call!(0x2fe840);
                pc = 0x31737b; continue;
            }
            0x31737b => {
                push!(ebx);
                push!(ebx);
                edx = esp.wrapping_add(0x2c);
                push!(edx);
                ecx = esp.wrapping_add(0x3c);
                push!(ecx);
                push!(eax);
                push!(0x1);
                call!(0xa6ec0);
                pc = 0x31738f; continue;
            }
            0x31738f => {
                edx = m.read32(0x7844a8);
                m.write32(edx.wrapping_add(0x4150), eax);
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x4150));
                edx = m.read32(ecx.wrapping_add(0x4));
                push!(ebx);
                push!(0x1);
                push!(edx);
                call!(0x966e0);
                pc = 0x3173b2; continue;
            }
            0x3173b2 => {
                eax = m.read32(0x778d34);
                eax = m.read32(eax.wrapping_add(0x88));
                eax = ((eax as i32).wrapping_mul((0xc4) as i32)) as u32;
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0x525010))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x28);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x6i32)) as u32) & 0xffu32);
                ecx = esp.wrapping_add(0x14);
                if ((fa & fb) == 0) { pc = 0x3173dc; continue; }
                pc = 0x3173d5; continue;
            }
            0x3173d5 => {
                eax = 0x7f7f7f7f;
                pc = 0x3173e2; continue;
            }
            0x3173dc => {
                eax = m.read32(eax.wrapping_add(0x524f5c));
                pc = 0x3173e2; continue;
            }
            0x3173e2 => {
                call!(0x317030);
                pc = 0x3173e7; continue;
            }
            0x3173e7 => {
                ecx = m.read32(0x7844a8);
                edx = m.read32(ecx.wrapping_add(0x4150));
                xmm3 = Xmm::scalar(m.read32(0x475d10));
                xmm4 = Xmm::scalar(m.read32(0x49f7e4));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x18)));
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x1c)));
                xh[1] = Xmm::scalar(m.read32(0x47d598));
                esi = 0x778ce0;
                m.write32(edx.wrapping_add(0x254), esi);
                eax = 0;
                ecx = 0x15;
                edi = esi;
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), eax); } edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep stosd, DF from entry.eflags
                ecx = 0;
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                m.write32(0x778d1c, xmm4.lanes[0]);
                xmm4 = Xmm::scalar(m.read32(0x47d1e4));
                m.write32(0x778d28, xmm4.lanes[0]);
                xmm4 = Xmm::scalar(m.read32(0x475d04));
                xh[0] = xmm0;
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(0x778ce0, xh[0].lanes[0]);
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = Xmm::scalar(m.read32(0x49f7e0));
                xh[0] = xmm1;
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(0x778ce4, xh[0].lanes[0]);
                xh[0] = xmm2;
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = Xmm::scalar(m.read32(0x475c78));
                m.write32(0x778ce8, xh[0].lanes[0]);
                xh[0] = Xmm::scalar(m.read32(0x475d40));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                m.write32(0x778d10, xmm3.lanes[0]);
                xmm3 = Xmm::default();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm4.lanes[0]) as f64;
                m.write32(0x778d14, xmm3.lanes[0]);
                m.write32(0x778d18, xmm3.lanes[0]);
                m.write32(0x778d20, xmm3.lanes[0]);
                m.write32(0x778d24, xmm3.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x3174df; continue; }
                pc = 0x3174dc; continue;
            }
            0x3174dc => {
                xmm0 = xmm4;
                pc = 0x3174df; continue;
            }
            0x3174df => {
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm4.lanes[0]) as f64;
                m.write32(0x778cec, xmm0.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x3174f7; continue; }
                pc = 0x3174f4; continue;
            }
            0x3174f4 => {
                xmm1 = xmm4;
                pc = 0x3174f7; continue;
            }
            0x3174f7 => {
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm4.lanes[0]) as f64;
                m.write32(0x778cf0, xmm1.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x31750f; continue; }
                pc = 0x31750c; continue;
            }
            0x31750c => {
                xmm2 = xmm4;
                pc = 0x31750f; continue;
            }
            0x31750f => {
                fa = ebp;
                fb = ((((0x46i32)) as u32) & 0xffffffffu32);
                m.write32(0x778cf4, xmm2.lanes[0]);
                m.write32(0x778cf8, xmm3.lanes[0]);
                m.write32(0x778cfc, xmm3.lanes[0]);
                m.write32(0x778d00, xmm3.lanes[0]);
                m.write32(0x778d04, xmm3.lanes[0]);
                m.write32(0x778d08, xmm3.lanes[0]);
                m.write32(0x778d0c, xmm3.lanes[0]);
                if (fa == fb) { pc = 0x317551; continue; }
                pc = 0x31754c; continue;
            }
            0x31754c => {
                fa = ebp;
                fb = ((((0x47i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3175b4; continue; }
                pc = 0x317551; continue;
            }
            0x317551 => {
                edx = m.read32(0x784020);
                eax = ebp;
                eax = ((eax as i32).wrapping_mul((0x7) as i32)) as u32;
                eax = eax.wrapping_add(edx);
                eax = ((eax as i32).wrapping_mul((0x1c) as i32)) as u32;
                eax = m.read32(eax.wrapping_add(0x524f68));
                push!(ebx);
                push!(ebx);
                ecx = esp.wrapping_add(0x28);
                push!(ecx);
                edx = esp.wrapping_add(0x38);
                push!(edx);
                push!(eax);
                push!(0x1);
                call!(0xa6ec0);
                pc = 0x31757b; continue;
            }
            0x31757b => {
                ecx = m.read32(0x7844a8);
                m.write32(ecx.wrapping_add(0x4154), eax);
                edx = m.read32(0x7844a8);
                eax = m.read32(edx.wrapping_add(0x4154));
                ecx = m.read32(eax.wrapping_add(0x4));
                push!(ebx);
                push!(0x1);
                push!(ecx);
                call!(0x966e0);
                pc = 0x31759f; continue;
            }
            0x31759f => {
                edx = m.read32(0x7844a8);
                eax = m.read32(edx.wrapping_add(0x4154));
                esp = esp.wrapping_add(0x24);
                m.write32(eax.wrapping_add(0x254), esi);
                pc = 0x3175b4; continue;
            }
            0x3175b4 => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x4150));
                fa = eax;
                fb = ebx;
                if (fa == fb) { pc = 0x31764e; continue; }
                pc = 0x3175c8; continue;
            }
            0x3175c8 => {
                edx = m.read32(eax.wrapping_add(0x28));
                fa = (((((((edx) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x31764e; continue; }
                pc = 0x3175d0; continue;
            }
            0x3175d0 => {
                ecx = ebp.wrapping_add(((0x11i32.wrapping_neg()) as u32));
                fa = ecx;
                fb = ((((0x6i32)) as u32) & 0xffffffffu32);
                if (fa >= fb) { pc = 0x3175ef; continue; }
                pc = 0x3175d8; continue;
            }
            0x3175d8 => {
                edx = ebp.wrapping_add(((0x11i32.wrapping_neg()) as u32));
                push!(edx);
                call!(0x2fe780);
                pc = 0x3175e1; continue;
            }
            0x3175e1 => {
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x10)));
                esp = esp.wrapping_add(0x4);
                m.write32(esp.wrapping_add(0x3c), xmm0.lanes[0]);
                pc = 0x3175ef; continue;
            }
            0x3175ef => {
                push!(ebp);
                call!(0x32d120);
                pc = 0x3175f5; continue;
            }
            0x3175f5 => {
                { let value=round_pc(fpu.st(0) * (f32::from_bits(m.read32(esp.wrapping_add(0x40))) as f64),fpu.control);fpu.stack[((fpu.top+0)&7) as usize]=value; }
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x4150));
                eax = m.read32(eax.wrapping_add(0x4));
                m.write32(esp.wrapping_add(0x14), (fpu.st(0) as f32).to_bits()); fpu.pop();
                edx = m.read32(eax.wrapping_add(0x4));
                esi = m.read32(edx.wrapping_add(0x4));
                esp = esp.wrapping_add(0x4);
                edi = 0;
                fa = esi;
                fb = ebx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x31764e; continue; }
                pc = 0x31761b; continue;
            }
            0x31761b => {
                esi = m.read32(esp.wrapping_add(0x10));
                pc = 0x317620; continue;
            }
            0x317620 => {
                eax = m.read32(eax.wrapping_add(0x10));
                push!(esi);
                push!(esi);
                eax = eax.wrapping_add(ebx);
                push!(esi);
                push!(eax);
                call!(0x153600);
                pc = 0x31762e; continue;
            }
            0x31762e => {
                ecx = m.read32(0x7844a8);
                edx = m.read32(ecx.wrapping_add(0x4150));
                eax = m.read32(edx.wrapping_add(0x4));
                edx = m.read32(eax.wrapping_add(0x4));
                esp = esp.wrapping_add(0x10);
                edi = edi.wrapping_add(1);
                ebx = ebx.wrapping_add(0x40);
                fa = edi;
                fb = m.read32(edx.wrapping_add(0x4));
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x317620; continue; }
                pc = 0x31764c; continue;
            }
            0x31764c => {
                ebx = 0;
                pc = 0x31764e; continue;
            }
            0x31764e => {
                eax = m.read32(ecx.wrapping_add(0x4154));
                fa = eax;
                fb = ebx;
                if (fa == fb) { pc = 0x3176be; continue; }
                pc = 0x317658; continue;
            }
            0x317658 => {
                ecx = m.read32(eax.wrapping_add(0x28));
                fa = (((((((ecx) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x3176be; continue; }
                pc = 0x317660; continue;
            }
            0x317660 => {
                push!(ebp);
                call!(0x32d120);
                pc = 0x317666; continue;
            }
            0x317666 => {
                { let value=round_pc(fpu.st(0) * (f32::from_bits(m.read32(esp.wrapping_add(0x40))) as f64),fpu.control);fpu.stack[((fpu.top+0)&7) as usize]=value; }
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x4154));
                eax = m.read32(ecx.wrapping_add(0x4));
                m.write32(esp.wrapping_add(0x40), (fpu.st(0) as f32).to_bits()); fpu.pop();
                edx = m.read32(eax.wrapping_add(0x4));
                ecx = m.read32(edx.wrapping_add(0x4));
                esp = esp.wrapping_add(0x4);
                edi = 0;
                fa = ecx;
                fb = ebx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x3176be; continue; }
                pc = 0x31768b; continue;
            }
            0x31768b => {
                esi = m.read32(esp.wrapping_add(0x3c));
                pc = 0x317690; continue;
            }
            0x317690 => {
                eax = m.read32(eax.wrapping_add(0x10));
                push!(esi);
                push!(esi);
                eax = eax.wrapping_add(ebx);
                push!(esi);
                push!(eax);
                call!(0x153600);
                pc = 0x31769e; continue;
            }
            0x31769e => {
                ecx = m.read32(0x7844a8);
                edx = m.read32(ecx.wrapping_add(0x4154));
                eax = m.read32(edx.wrapping_add(0x4));
                ecx = m.read32(eax.wrapping_add(0x4));
                edx = m.read32(ecx.wrapping_add(0x4));
                esp = esp.wrapping_add(0x10);
                edi = edi.wrapping_add(1);
                ebx = ebx.wrapping_add(0x40);
                fa = edi;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x317690; continue; }
                pc = 0x3176be; continue;
            }
            0x3176be => {
                call!(0x32d320);
                pc = 0x3176c3; continue;
            }
            0x3176c3 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x28);
                break 'run;
            }
            0x3176cb => {
                edx = m.read32(0x7844a8);
                eax = m.read32(edx.wrapping_add(0x4150));
                esi = 0;
                fa = eax;
                fb = esi;
                if (fa == fb) { pc = 0x31773c; continue; }
                pc = 0x3176dd; continue;
            }
            0x3176dd => {
                fa = ((((m.read16(eax.wrapping_add(0x94)))) as u32) & 0xffffu32);
                fb = ((((0x7a1i32)) as u32) & 0xffffu32);
                if (fa != fb) { pc = 0x3176f8; continue; }
                pc = 0x3176e8; continue;
            }
            0x3176e8 => {
                eax = m.read32(eax.wrapping_add(0x7c));
                fa = eax;
                fb = esi;
                if (fa == fb) { pc = 0x3176f8; continue; }
                pc = 0x3176ef; continue;
            }
            0x3176ef => {
                push!(eax);
                call!(0xff200);
                pc = 0x3176f5; continue;
            }
            0x3176f5 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x3176f8; continue;
            }
            0x3176f8 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x4150));
                push!(ecx);
                call!(0xa9070);
                pc = 0x317709; continue;
            }
            0x317709 => {
                edx = m.read32(0x7844a8);
                m.write32(edx.wrapping_add(0x4150), esi);
                eax = m.read32(0x7844a8);
                eax = m.read32(eax.wrapping_add(0x4154));
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = esi;
                if (fa == fb) { pc = 0x31773c; continue; }
                pc = 0x317727; continue;
            }
            0x317727 => {
                push!(eax);
                call!(0xa9070);
                pc = 0x31772d; continue;
            }
            0x31772d => {
                ecx = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x4);
                m.write32(ecx.wrapping_add(0x4154), esi);
                pc = 0x31773c; continue;
            }
            0x31773c => {
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

/// 0x00317750: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f317750<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x317750;
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
        { crate::pose::cover(0x317750, previous, pc); previous = pc; }
        match pc {
            0x317750 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                push!(esi);
                call!(0x71de0);
                pc = 0x31775b; continue;
            }
            0x31775b => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x317777; continue; }
                pc = 0x317762; continue;
            }
            0x317762 => {
                eax = m.read32(esi.wrapping_add(0x18c));
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x317777; continue; }
                pc = 0x31776d; continue;
            }
            0x31776d => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(esp.wrapping_add(0x4), eax);
                tail_to!(0x3170a0); break 'run;  // direct tail jump
            }
            0x317777 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00317780: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f317780<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x317780;
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
        { crate::pose::cover(0x317780, previous, pc); previous = pc; }
        match pc {
            0x317780 => {
                push!(esi);
                push!(edi);
                edi = m.read32(ecx.wrapping_add(0x8));
                eax = 0;
                esi = 0;
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x3177e1; continue; }
                pc = 0x31778d; continue;
            }
            0x31778d => {
                edx = m.read32(ecx.wrapping_add(0x4));
                push!(ebp);
                ebp = m.read32(0x784020);
                push!(ebx);
                pc = 0x3177a0; continue;
            }
            0x3177a0 => {
                ecx = m.read32(edx);
                eax = ecx;
                eax = ((eax as i32).wrapping_mul((0xc4) as i32)) as u32;
                ebx = 0;
                fa = m.read32(eax.wrapping_add(0x525010));
                fb = ebx;
                ebx = (ebx & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x3177da; continue; }
                pc = 0x3177ba; continue;
            }
            0x3177ba => {
                ecx = ((ecx as i32).wrapping_mul((0x7) as i32)) as u32;
                ecx = ecx.wrapping_add(ebp);
                ecx = ((ecx as i32).wrapping_mul((0x1c) as i32)) as u32;
                fa = m.read32(ecx.wrapping_add(0x524f64));
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3177da; continue; }
                pc = 0x3177cb; continue;
            }
            0x3177cb => {
                esi = esi.wrapping_add(1);
                edx = edx.wrapping_add(0x4);
                fa = esi;
                fb = edi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x3177a0; continue; }
                pc = 0x3177d3; continue;
            }
            0x3177d3 => {
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x3177da => {
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0x1;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x3177e1; continue;
            }
            0x3177e1 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x003177F0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3177f0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3177f0;
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
        { crate::pose::cover(0x3177f0, previous, pc); previous = pc; }
        match pc {
            0x3177f0 => {
                push!(ecx);
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0xc));
                ecx = ebp;
                call!(0x317780);
                pc = 0x3177fd; continue;
            }
            0x3177fd => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3178f7; continue; }
                pc = 0x317805; continue;
            }
            0x317805 => {
                eax = m.read32(ebp);
                push!(esi);
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(eax);
                call!(0x3d7b0);
                pc = 0x317817; continue;
            }
            0x317817 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79460);
                pc = 0x317820; continue;
            }
            0x317820 => {
                edx = m.read32(0x7844a8);
                esi = m.read32(edx.wrapping_add(0x254));
                ecx = 0;
                fa = esi;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                ecx = (ecx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                ecx = ecx.wrapping_add(0x4);
                push!(ecx);
                push!(0x1);
                push!(0x1);
                call!(0x78320);
                pc = 0x317841; continue;
            }
            0x317841 => {
                ecx = m.read32(ebp.wrapping_add(0x8));
                eax = 0;
                esp = esp.wrapping_add(0x20);
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x8), eax);
                if (fas <= fbs) { pc = 0x3178e2; continue; }
                pc = 0x317855; continue;
            }
            0x317855 => {
                push!(ebx);
                push!(edi);
                pc = 0x317857; continue;
            }
            0x317857 => {
                ecx = m.read32(ebp.wrapping_add(0x4));
                edi = m.read32(ecx.wrapping_add(eax.wrapping_mul(0x4)));
                esi = edi;
                esi = ((esi as i32).wrapping_mul((0xc4) as i32)) as u32;
                esi = esi.wrapping_add(0x524f58);
                eax = (((((m.read16(esi.wrapping_add(0x2)) as i16)) as i32)) as u32);
                eax = eax.wrapping_add(eax.wrapping_mul(0x2));
                eax = eax.wrapping_shl((((((((0x3i32)) as u32) & 0x1fu32))) as u32 & 31));
                ebx = m.read32(eax.wrapping_add(0x524bec));
                eax = m.read32(eax.wrapping_add(0x524bf0));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31788f; continue; }
                pc = 0x317885; continue;
            }
            0x317885 => {
                call_to!(eax);
                pc = 0x317887; continue;
            }
            0x317887 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31788f; continue; }
                pc = 0x31788b; continue;
            }
            0x31788b => {
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x317893; continue; }
                pc = 0x31788f; continue;
            }
            0x31788f => {
                ebx = m.read32(esp.wrapping_add(0x1c));
                pc = 0x317893; continue;
            }
            0x317893 => {
                edx = m.read32(0x784020);
                edx = ((edx as i32).wrapping_mul((0x1c) as i32)) as u32;
                fa = m.read32(edx.wrapping_add(esi).wrapping_add(0xc));
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3178cc; continue; }
                pc = 0x3178a3; continue;
            }
            0x3178a3 => {
                push!(0x316110);
                push!(ebx);
                push!(0x412ccccc);
                push!(0x410a3d71);
                push!(0);
                push!(edi);
                push!(0);
                call!(0x792b0);
                pc = 0x3178bd; continue;
            }
            0x3178bd => {
                eax = m.read32(esi.wrapping_add(0xbc));
                push!(eax);
                call!(0x716d0);
                pc = 0x3178c9; continue;
            }
            0x3178c9 => {
                esp = esp.wrapping_add(0x20);
                pc = 0x3178cc; continue;
            }
            0x3178cc => {
                eax = m.read32(esp.wrapping_add(0x10));
                ecx = m.read32(ebp.wrapping_add(0x8));
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x10), eax);
                if (fas < fbs) { pc = 0x317857; continue; }
                pc = 0x3178e0; continue;
            }
            0x3178e0 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x3178e2; continue;
            }
            0x3178e2 => {
                call!(0x78380);
                pc = 0x3178e7; continue;
            }
            0x3178e7 => {
                call!(0x783d0);
                pc = 0x3178ec; continue;
            }
            0x3178ec => {
                push!(0);
                call!(0x795b0);
                pc = 0x3178f3; continue;
            }
            0x3178f3 => {
                esp = esp.wrapping_add(0x4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x3178f7; continue;
            }
            0x3178f7 => {
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00317900: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f317900<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x317900;
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
        { crate::pose::cover(0x317900, previous, pc); previous = pc; }
        match pc {
            0x317900 => {
                eax = m.read32(esp.wrapping_add(0x4));
                eax = eax.wrapping_sub(0);
                fa = eax;
                push!(esi);
                if ((fa == 0)) { pc = 0x317942; continue; }
                pc = 0x31790a; continue;
            }
            0x31790a => {
                eax = eax.wrapping_sub(1);
                fa = eax;
                if ((fa != 0)) { pc = 0x31795f; continue; }
                pc = 0x31790d; continue;
            }
            0x31790d => {
                esi = m.read32(esp.wrapping_add(0xc));
                push!(esi);
                push!(0x52dcdc);
                call!(0x3177f0);
                pc = 0x31791c; continue;
            }
            0x31791c => {
                push!(esi);
                push!(0x52dc64);
                call!(0x3177f0);
                pc = 0x317927; continue;
            }
            0x317927 => {
                push!(esi);
                push!(0x52dc70);
                call!(0x3177f0);
                pc = 0x317932; continue;
            }
            0x317932 => {
                push!(esi);
                push!(0x52dc7c);
                call!(0x3177f0);
                pc = 0x31793d; continue;
            }
            0x31793d => {
                esp = esp.wrapping_add(0x20);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x317942 => {
                esi = m.read32(esp.wrapping_add(0xc));
                push!(esi);
                push!(0x52ddd4);
                call!(0x3177f0);
                pc = 0x317951; continue;
            }
            0x317951 => {
                push!(esi);
                push!(0x52dcb8);
                call!(0x3177f0);
                pc = 0x31795c; continue;
            }
            0x31795c => {
                esp = esp.wrapping_add(0x10);
                pc = 0x31795f; continue;
            }
            0x31795f => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00317970: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f317970<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x317970;
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
        { crate::pose::cover(0x317970, previous, pc); previous = pc; }
        match pc {
            0x317970 => {
                eax = m.read32(esp.wrapping_add(0x4));
                eax = eax.wrapping_sub(0);
                fa = eax;
                push!(esi);
                if ((fa == 0)) { pc = 0x3179a7; continue; }
                pc = 0x31797a; continue;
            }
            0x31797a => {
                eax = eax.wrapping_sub(1);
                fa = eax;
                if ((fa != 0)) { pc = 0x3179c4; continue; }
                pc = 0x31797d; continue;
            }
            0x31797d => {
                esi = m.read32(esp.wrapping_add(0xc));
                push!(esi);
                push!(0x52dc64);
                call!(0x3177f0);
                pc = 0x31798c; continue;
            }
            0x31798c => {
                push!(esi);
                push!(0x52dc70);
                call!(0x3177f0);
                pc = 0x317997; continue;
            }
            0x317997 => {
                push!(esi);
                push!(0x52dc7c);
                call!(0x3177f0);
                pc = 0x3179a2; continue;
            }
            0x3179a2 => {
                esp = esp.wrapping_add(0x18);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x3179a7 => {
                esi = m.read32(esp.wrapping_add(0xc));
                push!(esi);
                push!(0x52ddd4);
                call!(0x3177f0);
                pc = 0x3179b6; continue;
            }
            0x3179b6 => {
                push!(esi);
                push!(0x52dcb8);
                call!(0x3177f0);
                pc = 0x3179c1; continue;
            }
            0x3179c1 => {
                esp = esp.wrapping_add(0x10);
                pc = 0x3179c4; continue;
            }
            0x3179c4 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00317B40: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f317b40<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x317b40;
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
        { crate::pose::cover(0x317b40, previous, pc); previous = pc; }
        match pc {
            0x317b40 => {
                eax = m.read32(0x7844a8);
                eax = m.read32(eax.wrapping_add(0x2f1c));
                ecx = m.read32(eax);
                ecx = ((ecx as i32).wrapping_mul((0xc4) as i32)) as u32;
                edx = (((((m.read16(ecx.wrapping_add(0x524f58)) as i16)) as i32)) as u32);
                push!(edx);
                call!(0x3d7b0);
                pc = 0x317b60; continue;
            }
            0x317b60 => {
                esp = esp.wrapping_add(0x4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00317B70: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f317b70<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x317b70;
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
        { crate::pose::cover(0x317b70, previous, pc); previous = pc; }
        match pc {
            0x317b70 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f1c));
                eax = m.read32(ecx);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00317B80: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f317b80<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x317b80;
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
        { crate::pose::cover(0x317b80, previous, pc); previous = pc; }
        match pc {
            0x317b80 => {
                edx = m.read32(0x7844a8);
                ecx = m.read32(edx.wrapping_add(0x2f78));
                ecx = ecx.wrapping_add(ecx.wrapping_mul(0x2));
                ecx = ((m.read8(edx.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x3077))) as u8 as u32);
                eax = 0;
                ecx = ecx.wrapping_sub(eax);
                fa = ecx;
                if ((fa == 0)) { pc = 0x317bb1; continue; }
                pc = 0x317b9d; continue;
            }
            0x317b9d => {
                ecx = ecx.wrapping_sub(1);
                fa = ecx;
                if ((fa != 0)) { pc = 0x317bc1; continue; }
                pc = 0x317ba0; continue;
            }
            0x317ba0 => {
                edx = (((((m.read16(0x5285b4) as i16)) as i32)) as u32);
                push!(edx);
                call!(0x3d7b0);
                pc = 0x317bad; continue;
            }
            0x317bad => {
                esp = esp.wrapping_add(0x4);
                break 'run;
            }
            0x317bb1 => {
                eax = (((((m.read16(0x5284f0) as i16)) as i32)) as u32);
                push!(eax);
                call!(0x3d7b0);
                pc = 0x317bbe; continue;
            }
            0x317bbe => {
                esp = esp.wrapping_add(0x4);
                pc = 0x317bc1; continue;
            }
            0x317bc1 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00317BD0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f317bd0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x317bd0;
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
        { crate::pose::cover(0x317bd0, previous, pc); previous = pc; }
        match pc {
            0x317bd0 => {
                edx = m.read32(0x7844a8);
                ecx = m.read32(edx.wrapping_add(0x2f78));
                ecx = ecx.wrapping_add(ecx.wrapping_mul(0x2));
                ecx = ((m.read8(edx.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x3077))) as u8 as u32);
                eax = (eax | 0xffffffffu32);
                ecx = ecx.wrapping_sub(0);
                fa = ecx;
                if ((fa == 0)) { pc = 0x317bf8; continue; }
                pc = 0x317bef; continue;
            }
            0x317bef => {
                ecx = ecx.wrapping_sub(1);
                fa = ecx;
                if ((fa != 0)) { pc = 0x317bfd; continue; }
                pc = 0x317bf2; continue;
            }
            0x317bf2 => {
                eax = 0x47;
                break 'run;
            }
            0x317bf8 => {
                eax = 0x46;
                pc = 0x317bfd; continue;
            }
            0x317bfd => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00317C00: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f317c00<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x317c00;
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
        { crate::pose::cover(0x317c00, previous, pc); previous = pc; }
        match pc {
            0x317c00 => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x2f20));
                fa = eax;
                fb = m.read32(ecx.wrapping_add(0x280));
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x317c17; continue; }
                pc = 0x317c14; continue;
            }
            0x317c14 => {
                eax = (((((eax as i32)).wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                pc = 0x317c17; continue;
            }
            0x317c17 => {
                eax = ((eax as i32).wrapping_mul((0xe4) as i32)) as u32;
                eax = eax.wrapping_add(ecx).wrapping_add(0x288);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00317C30: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f317c30<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x317c30;
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
        { crate::pose::cover(0x317c30, previous, pc); previous = pc; }
        match pc {
            0x317c30 => {
                eax = 0x38;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00317C40: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f317c40<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x317c40;
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
        { crate::pose::cover(0x317c40, previous, pc); previous = pc; }
        match pc {
            0x317c40 => {
                eax = m.read32(0x7844a8);
                eax = m.read32(eax.wrapping_add(0x2f18));
                eax = m.read32(eax.wrapping_add(0x10));
                ecx = eax.wrapping_add(eax.wrapping_mul(0x4));
                ecx = ecx.wrapping_shl((((((((0x6i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = m.read32(ecx.wrapping_add(0x529518));
                push!(edx);
                call!(0x3d7b0);
                pc = 0x317c60; continue;
            }
            0x317c60 => {
                esp = esp.wrapping_add(0x4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00317D60: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f317d60<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x317d60;
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
        { crate::pose::cover(0x317d60, previous, pc); previous = pc; }
        match pc {
            0x317d60 => {
                eax = m.read32(0x783b8c);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00318280: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f318280<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x318280;
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
        { crate::pose::cover(0x318280, previous, pc); previous = pc; }
        match pc {
            0x318280 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f1c));
                fa = m.read32(ecx);
                fb = ((((0x43i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x318295; continue; }
                pc = 0x318290; continue;
            }
            0x318290 => {
                tail_to!(0x2fb3c0); break 'run;  // direct tail jump
            }
            0x318295 => {
                eax = m.read32(0x7842a8);
                push!(0x52de4c);
                push!(eax);
                call!(0x79b70);
                pc = 0x3182a5; continue;
            }
            0x3182a5 => {
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x003182B0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3182b0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3182b0;
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
        { crate::pose::cover(0x3182b0, previous, pc); previous = pc; }
        match pc {
            0x3182b0 => {
                eax = m.read32(0x7844a8);
                eax = m.read32(eax.wrapping_add(0x2f1c));
                esp = esp.wrapping_sub(0x3c);
                push!(esi);
                push!(edi);
                ecx = 0xf;
                esi = eax;
                edi = esp.wrapping_add(0x8);
                { let step = if entry.eflags & 0x400 == 0 { 4u32 } else { 0u32.wrapping_sub(4) }; for i in 0..ecx { let value=m.read32(esi.wrapping_add(i.wrapping_mul(step)));m.write32(edi.wrapping_add(i.wrapping_mul(step)),value); } esi=esi.wrapping_add(ecx.wrapping_mul(step));edi=edi.wrapping_add(ecx.wrapping_mul(step));ecx=0; }
                ecx = m.read32(eax.wrapping_add(0x34));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ecx = m.read32(eax.wrapping_add(0x2c));
                if (fa != fb) { pc = 0x3182dc; continue; }
                pc = 0x3182d7; continue;
            }
            0x3182d7 => {
                ecx = (ecx & 0xfffffff7u32);
                pc = 0x3182df; continue;
            }
            0x3182dc => {
                ecx = (ecx | 0x8);
                pc = 0x3182df; continue;
            }
            0x3182df => {
                m.write32(eax.wrapping_add(0x2c), ecx);
                ecx = m.read32(0x7844a8);
                ecx = m.read32(ecx.wrapping_add(0x2f1c));
                fa = m.read32(ecx);
                fb = ((((0x37i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x318302; continue; }
                pc = 0x3182f3; continue;
            }
            0x3182f3 => {
                esi = m.read32(eax.wrapping_add(0x30));
                edx = 0;
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                edx = (edx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                edx = edx.wrapping_add(0x35);
                m.write32(ecx, edx);
                pc = 0x318302; continue;
            }
            0x318302 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f1c));
                eax = m.read32(ecx);
                fa = eax;
                fb = m.read32(esp.wrapping_add(0x8));
                if (fa == fb) { pc = 0x318324; continue; }
                pc = 0x318315; continue;
            }
            0x318315 => {
                fa = eax;
                fb = ((((0x36i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x318324; continue; }
                pc = 0x31831a; continue;
            }
            0x31831a => {
                push!(0x5);
                call!(0x2fb930);
                pc = 0x318321; continue;
            }
            0x318321 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x318324; continue;
            }
            0x318324 => {
                esi = m.read32(esp.wrapping_add(0x48));
                push!(0);
                push!(esi);
                call!(0x71880);
                pc = 0x318330; continue;
            }
            0x318330 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x318340; continue; }
                pc = 0x318337; continue;
            }
            0x318337 => {
                push!(esi);
                call!(0x78980);
                pc = 0x31833d; continue;
            }
            0x31833d => {
                esp = esp.wrapping_add(0x4);
                pc = 0x318340; continue;
            }
            0x318340 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x3c);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00318AF0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f318af0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x318af0;
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
        { crate::pose::cover(0x318af0, previous, pc); previous = pc; }
        match pc {
            0x318af0 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f78));
                edx = m.read32(esp.wrapping_add(0x4));
                push!(esi);
                push!(edi);
                ecx = ecx.wrapping_add(ecx.wrapping_mul(0x2));
                push!(0);
                push!(edx);
                esi = eax.wrapping_add(0x2f4c);
                edi = eax.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x3070);
                call!(0x71880);
                pc = 0x318b19; continue;
            }
            0x318b19 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x318b5c; continue; }
                pc = 0x318b20; continue;
            }
            0x318b20 => {
                eax = m.read32(esi.wrapping_add(0x8));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x318b33; continue; }
                pc = 0x318b27; continue;
            }
            0x318b27 => {
                ecx = eax.wrapping_add(((0x1i32.wrapping_neg()) as u32));
                eax = 0x1;
                if !(((((((((ecx) as u8))) as u32) & 0x1fu32))) != 0) { fa = if ((fa == fb)) { 0x0u32 } else { (if ((((((((fas)) as u32).wrapping_sub((((fbs)) as u32))).wrapping_shr((0x1f & 31))) != 0))) { 0x80000000u32 } else { 0x1u32 }) }; fas = ((fa) as i32); }
                eax = eax.wrapping_shl((((((((((ecx) as u8))) as u32) & 0x1fu32))) as u32 & 31));
                if (((((((((ecx) as u8))) as u32) & 0x1fu32))) != 0) { fa = eax; fas = (fa as i32); }
                pc = 0x318b35; continue;
            }
            0x318b33 => {
                eax = 0;
                pc = 0x318b35; continue;
            }
            0x318b35 => {
                m.write8(edi.wrapping_add(0x8), (((((eax) as u8)) as u32)) as u8);
                eax = m.read32(esi.wrapping_add(0xc));
                ecx = m.read32(esi.wrapping_add(0x10));
                edx = (edx & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(0x4))) as u32)) & 0xff);
                cf = (((((eax) != 0))) as i32);
                eax = (((((eax) as i32).wrapping_neg())) as u32);
                eax = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                eax = (eax & 0x8);
                cf = (((((ecx) != 0))) as i32);
                ecx = (((((ecx) as i32).wrapping_neg())) as u32);
                ecx = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                ecx = (ecx & 0x4);
                edx = (edx & 0xffff_ff00) | (((((((((edx) as u8)) as i32) | ((((ecx) as u8)) as i32))) as u32)) & 0xff);
                edx = (edx & 0xffff_ff00) | (((((((((edx) as u8)) as i32) | ((((eax) as u8)) as i32))) as u32)) & 0xff);
                m.write32(esi.wrapping_add(0xc), eax);
                m.write32(esi.wrapping_add(0x10), ecx);
                m.write8(edi.wrapping_add(0x9), (((((edx) as u8)) as u32)) as u8);
                pc = 0x318b5c; continue;
            }
            0x318b5c => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00318B60: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f318b60<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x318b60;
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
        { crate::pose::cover(0x318b60, previous, pc); previous = pc; }
        match pc {
            0x318b60 => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x2f78));
                eax = eax.wrapping_add(eax.wrapping_mul(0x2));
                eax = ecx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x3070);
                fa = ((((m.read8(eax.wrapping_add(0x9)))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                ecx = 0;
                ecx = (ecx & 0xffff_ff00) | ((((if (((fa & fb) != 0)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                edx = 0;
                ecx = ecx.wrapping_add(1);
                m.write32(esi.wrapping_add(0x4), ecx);
                edx = (edx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0x9))) as u32)) & 0xff);
                ecx = 0;
                edx = edx.wrapping_shr((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = (edx & 0x1);
                m.write32(esi.wrapping_add(0x10), edx);
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0x9))) as u32)) & 0xff);
                ecx = ecx.wrapping_shr((((((((0x3i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx & 0x1);
                m.write32(esi.wrapping_add(0xc), ecx);
                edx = ((m.read8(eax.wrapping_add(0x8))) as u8 as u32);
                ecx = 0;
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x318bc6; continue; }
                pc = 0x318bac; continue;
            }
            0x318bac => {
                fa = edx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x318bc6; continue; }
                pc = 0x318bb1; continue;
            }
            0x318bb1 => {
                eax = 0x1;
                fa = edx;
                fb = eax;
                pc = 0x318bb8; continue;
            }
            0x318bb8 => {
                if (fa == fb) { pc = 0x318bcc; continue; }
                pc = 0x318bba; continue;
            }
            0x318bba => {
                ecx = ecx.wrapping_add(1);
                fa = ecx;
                fas = (fa as i32);
                eax = 0x1;
                if !(((((((((ecx) as u8))) as u32) & 0x1fu32))) != 0) { fa = if (((fa == 0))) { 0x0u32 } else { (if (((fas < 0i32))) { 0x80000000u32 } else { 0x1u32 }) }; fas = ((fa) as i32); }
                eax = eax.wrapping_shl((((((((((ecx) as u8))) as u32) & 0x1fu32))) as u32 & 31));
                if (((((((((ecx) as u8))) as u32) & 0x1fu32))) != 0) { fa = eax; fas = (fa as i32); }
                fa = edx;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x318bb8; continue; }
                pc = 0x318bc6; continue;
            }
            0x318bc6 => {
                eax = 0;
                m.write32(esi.wrapping_add(0x8), eax);
                break 'run;
            }
            0x318bcc => {
                eax = ecx.wrapping_add(0x1);
                m.write32(esi.wrapping_add(0x8), eax);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00318E00: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f318e00<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x318e00;
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
        { crate::pose::cover(0x318e00, previous, pc); previous = pc; }
        match pc {
            0x318e00 => {
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x8));
                push!(0);
                push!(edi);
                call!(0x75580);
                pc = 0x318e0d; continue;
            }
            0x318e0d => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x318e32; continue; }
                pc = 0x318e14; continue;
            }
            0x318e14 => {
                push!(0x1);
                push!(edi);
                call!(0x75580);
                pc = 0x318e1c; continue;
            }
            0x318e1c => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x318e32; continue; }
                pc = 0x318e23; continue;
            }
            0x318e23 => {
                push!(0x2);
                push!(edi);
                call!(0x75580);
                pc = 0x318e2b; continue;
            }
            0x318e2b => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x318e97; continue; }
                pc = 0x318e32; continue;
            }
            0x318e32 => {
                eax = m.read32(0x7844a8);
                push!(esi);
                esi = m.read32(eax.wrapping_add(0x2f1c));
                call!(0x316d10);
                pc = 0x318e43; continue;
            }
            0x318e43 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x318e8d; continue; }
                pc = 0x318e47; continue;
            }
            0x318e47 => {
                ecx = m.read32(esi.wrapping_add(0x18));
                m.write32(eax.wrapping_add(0x18), ecx);
                edx = m.read32(esi.wrapping_add(0x1c));
                m.write32(eax.wrapping_add(0x1c), edx);
                ecx = m.read32(esi.wrapping_add(0x20));
                fa = ecx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x318e86; continue; }
                pc = 0x318e5b; continue;
            }
            0x318e5b => {
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x318e73; continue; }
                pc = 0x318e5f; continue;
            }
            0x318e5f => {
                ecx = ecx.wrapping_sub(1);
                cf = (((((ecx) != 0))) as i32);
                ecx = (((((ecx) as i32).wrapping_neg())) as u32);
                ecx = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                push!(edi);
                m.write32(eax.wrapping_add(0x20), ecx);
                call!(0x78980);
                pc = 0x318e6d; continue;
            }
            0x318e6d => {
                esp = esp.wrapping_add(0x4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x318e73 => {
                push!(edi);
                m.write32(eax.wrapping_add(0x20), 0x1);
                call!(0x78980);
                pc = 0x318e80; continue;
            }
            0x318e80 => {
                esp = esp.wrapping_add(0x4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x318e86 => {
                m.write32(eax.wrapping_add(0x20), 0xffffffffu32);
                pc = 0x318e8d; continue;
            }
            0x318e8d => {
                push!(edi);
                call!(0x78980);
                pc = 0x318e93; continue;
            }
            0x318e93 => {
                esp = esp.wrapping_add(0x4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x318e97; continue;
            }
            0x318e97 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x003194C0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3194c0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3194c0;
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
        { crate::pose::cover(0x3194c0, previous, pc); previous = pc; }
        match pc {
            0x3194c0 => {
                eax = m.read32(esp.wrapping_add(0x4));
                ecx = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x783a80));
                ecx = (ecx & 0xff7f7f00u32);
                eax = ecx.wrapping_add(ecx).wrapping_add(0xff);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x003194E0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3194e0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3194e0;
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
        { crate::pose::cover(0x3194e0, previous, pc); previous = pc; }
        match pc {
            0x3194e0 => {
                ecx = m.read32(esp.wrapping_add(0x4));
                eax = 0;
                ecx = (ecx & 0xffffff00u32);
                pc = 0x3194f0; continue;
            }
            0x3194f0 => {
                edx = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x783a80));
                edx = (edx & 0xffffff00u32);
                fa = edx;
                fb = ecx;
                if (fa == fb) { pc = 0x319509; continue; }
                pc = 0x319501; continue;
            }
            0x319501 => {
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ((((0x40i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x3194f0; continue; }
                pc = 0x319507; continue;
            }
            0x319507 => {
                eax = (eax & 0xffff_ff00) | ((0) & 0xff);
                pc = 0x319509; continue;
            }
            0x319509 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00319510: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f319510<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x319510;
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
        { crate::pose::cover(0x319510, previous, pc); previous = pc; }
        match pc {
            0x319510 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f18));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ecx = m.read32(esp.wrapping_add(0x4));
                if (fa == fb) { pc = 0x319545; continue; }
                pc = 0x319523; continue;
            }
            0x319523 => {
                eax = m.read32(ecx.wrapping_add(0x4));
                fa = eax;
                fb = ((((0x52e020i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x319545; continue; }
                pc = 0x31952d; continue;
            }
            0x31952d => {
                fa = eax;
                fb = ((((0x52e030i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x319545; continue; }
                pc = 0x319534; continue;
            }
            0x319534 => {
                fa = eax;
                fb = ((((0x52e040i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x319545; continue; }
                pc = 0x31953b; continue;
            }
            0x31953b => {
                call!(0x2fafa0);
                pc = 0x319540; continue;
            }
            0x319540 => {
                tail_to!(0x2f7ab0); break 'run;  // direct tail jump
            }
            0x319545 => {
                m.write32(esp.wrapping_add(0x4), ecx);
                tail_to!(0x7a7a0); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00319B50: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f319b50<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x319b50;
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
        { crate::pose::cover(0x319b50, previous, pc); previous = pc; }
        match pc {
            0x319b50 => {
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x8));
                push!(0x1);
                push!(edi);
                call!(0x71880);
                pc = 0x319b5d; continue;
            }
            0x319b5d => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x319bc0; continue; }
                pc = 0x319b64; continue;
            }
            0x319b64 => {
                eax = ((m.read8(0x783a60)) as u8 as u32);
                push!(esi);
                esi = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x783a80));
                push!(0x783b80);
                call!(0x154b50);
                pc = 0x319b7d; continue;
            }
            0x319b7d => {
                ecx = ((m.read8(0x783a60)) as u8 as u32);
                esp = esp.wrapping_add(0x4);
                fa = esi;
                fb = eax;
                m.write32(ecx.wrapping_mul(0x4).wrapping_add(0x783a80), eax);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                if (fa == fb) { pc = 0x319bc0; continue; }
                pc = 0x319b93; continue;
            }
            0x319b93 => {
                edx = m.read32(0x7844a8);
                fa = ((((m.read8(edx.wrapping_add(0x238)))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x319bc0; continue; }
                pc = 0x319ba2; continue;
            }
            0x319ba2 => {
                push!(0xb);
                call!(0x3370a0);
                pc = 0x319ba9; continue;
            }
            0x319ba9 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x238));
                esp = esp.wrapping_add(0x4);
                ecx = (ecx | 0x20);
                m.write32(eax.wrapping_add(0x238), ecx);
                pc = 0x319bc0; continue;
            }
            0x319bc0 => {
                push!(edi);
                call!(0x7a7a0);
                pc = 0x319bc6; continue;
            }
            0x319bc6 => {
                esp = esp.wrapping_add(0x4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00319F40: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f319f40<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x319f40;
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
        { crate::pose::cover(0x319f40, previous, pc); previous = pc; }
        match pc {
            0x319f40 => {
                eax = m.read32(0x78400c);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x319fb5; continue; }
                pc = 0x319f49; continue;
            }
            0x319f49 => {
                ecx = m.read32(esp.wrapping_add(0x4));
                xmm2 = Xmm::scalar(m.read32(0x49f7f0));
                xmm3 = Xmm::scalar(m.read32(0x49f7ec));
                xmm1 = Xmm::scalar(m.read32(0x475c78));
                xmm4 = Xmm::default();
                push!(esi);
                edx = (edx & 0xffff_ff00) | ((0x2) & 0xff);
                pc = 0x319f70; continue;
            }
            0x319f70 => {
                esi = m.read32(0x7844a8);
                fa = eax;
                fb = m.read32(esi.wrapping_add(0x2f18));
                if (fa == fb) { pc = 0x319f83; continue; }
                pc = 0x319f7e; continue;
            }
            0x319f7e => {
                fa = ((((m.read8(eax.wrapping_add(0x14)))) as u32) & 0xffu32);
                fb = ((((((edx) as u8))) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x319fad; continue; }
                pc = 0x319f83; continue;
            }
            0x319f83 => {
                fa = ecx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                xmm0 = xmm2;
                if (fa == fb) { pc = 0x319f8e; continue; }
                pc = 0x319f8b; continue;
            }
            0x319f8b => {
                xmm0 = xmm3;
                pc = 0x319f8e; continue;
            }
            0x319f8e => {
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x28)))).to_bits();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                m.write32(eax.wrapping_add(0x28), xmm0.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x319fa2; continue; }
                pc = 0x319f9d; continue;
            }
            0x319f9d => {
                m.write32(eax.wrapping_add(0x28), xmm1.lanes[0]);
                pc = 0x319fa2; continue;
            }
            0x319fa2 => {
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(eax.wrapping_add(0x28))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x319fad; continue; }
                pc = 0x319fa8; continue;
            }
            0x319fa8 => {
                m.write32(eax.wrapping_add(0x28), xmm4.lanes[0]);
                pc = 0x319fad; continue;
            }
            0x319fad => {
                eax = m.read32(eax.wrapping_add(0x40));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x319f70; continue; }
                pc = 0x319fb4; continue;
            }
            0x319fb4 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x319fb5; continue;
            }
            0x319fb5 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0031A130: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31a130<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31a130;
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
        { crate::pose::cover(0x31a130, previous, pc); previous = pc; }
        match pc {
            0x31a130 => {
                eax = m.read32(esp.wrapping_add(0x4));
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0x10));
                push!(ebp);
                ebp = 0;
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                push!(edi);
                ecx = eax.wrapping_add(0x18);
                edi = ebx;
                if (fa != fb) { pc = 0x31a14a; continue; }
                pc = 0x31a146; continue;
            }
            0x31a146 => {
                edi = esp.wrapping_add(0x14);
                pc = 0x31a14a; continue;
            }
            0x31a14a => {
                fa = ((((m.read8(eax.wrapping_add(0x14)))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x31a18c; continue; }
                pc = 0x31a150; continue;
            }
            0x31a150 => {
                push!(esi);
                esi = m.read32(0x78400c);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31a1a8; continue; }
                pc = 0x31a15b; continue;
            }
            0x31a15b => {
                ebp = m.read32(esp.wrapping_add(0x20));
                pc = 0x31a160; continue;
            }
            0x31a160 => {
                fa = ((((m.read8(esi.wrapping_add(0x14)))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                eax = esi.wrapping_add(0x18);
                if ((fa & fb) == 0) { pc = 0x31a174; continue; }
                pc = 0x31a169; continue;
            }
            0x31a169 => {
                push!(edi);
                push!(eax);
                call_to!(ebp);
                pc = 0x31a16d; continue;
            }
            0x31a16d => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31a182; continue; }
                pc = 0x31a174; continue;
            }
            0x31a174 => {
                esi = m.read32(esi.wrapping_add(0x40));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31a160; continue; }
                pc = 0x31a17b; continue;
            }
            0x31a17b => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x31a182 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0x1;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x31a18c => {
                push!(edi);
                push!(ecx);
                call_to!(m.read32(esp.wrapping_add(0x24)));
                pc = 0x31a192; continue;
            }
            0x31a192 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31a1a2; continue; }
                pc = 0x31a199; continue;
            }
            0x31a199 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0x1;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x31a1a2 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = ebp;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x31a1a8 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = ebp;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031A1B0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31a1b0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31a1b0;
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
        { crate::pose::cover(0x31a1b0, previous, pc); previous = pc; }
        match pc {
            0x31a1b0 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                eax = (eax & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(0x14))) as u32)) & 0xff);
                push!(edi);
                edi = 0;
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x31a22f; continue; }
                pc = 0x31a1bf; continue;
            }
            0x31a1bf => {
                esi = m.read32(0x78400c);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31a22a; continue; }
                pc = 0x31a1c9; continue;
            }
            0x31a1c9 => {
                pc = 0x31a1d0; continue;
            }
            0x31a1d0 => {
                fa = ((((m.read8(esi.wrapping_add(0x14)))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x31a223; continue; }
                pc = 0x31a1d6; continue;
            }
            0x31a1d6 => {
                call!(0x3118f0);
                pc = 0x31a1db; continue;
            }
            0x31a1db => {
                ecx = ((m.read8(esi.wrapping_add(0x18))) as u8 as u32);
                fa = ecx;
                fb = eax;
                if (fa != fb) { pc = 0x31a274; continue; }
                pc = 0x31a1e7; continue;
            }
            0x31a1e7 => {
                eax = m.read32(esi.wrapping_add(0x1c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31a274; continue; }
                pc = 0x31a1f2; continue;
            }
            0x31a1f2 => {
                eax = m.read32(esi.wrapping_add(0x20));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31a274; continue; }
                pc = 0x31a1f9; continue;
            }
            0x31a1f9 => {
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x24)));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x475d4c)) as f64);
                eax = (((eax & 0xffff00ffu32)) | ((((((if ((((fca == fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x40u32 } else { 0x0u32 }) | (if ((((fca != fca) || (fcb != fcb)))) { 0x4u32 } else { 0x0u32 })) | (if ((((fca < fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x1u32 } else { 0x0u32 })) | 0x2u32)).wrapping_shl((0x8 & 31))));
                fa = (((((((eax) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x44i32)) as u32) & 0xffu32);
                if (((((fa) & (fb))) as u8).count_ones() % 2 == 0) { pc = 0x31a274; continue; }
                pc = 0x31a20b; continue;
            }
            0x31a20b => {
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x28)));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x475cac)) as f64);
                eax = (((eax & 0xffff00ffu32)) | ((((((if ((((fca == fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x40u32 } else { 0x0u32 }) | (if ((((fca != fca) || (fcb != fcb)))) { 0x4u32 } else { 0x0u32 })) | (if ((((fca < fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x1u32 } else { 0x0u32 })) | 0x2u32)).wrapping_shl((0x8 & 31))));
                fa = (((((((eax) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x44i32)) as u32) & 0xffu32);
                if (((((fa) & (fb))) as u8).count_ones() % 2 == 0) { pc = 0x31a274; continue; }
                pc = 0x31a21d; continue;
            }
            0x31a21d => {
                fa = ((((m.read8(esi.wrapping_add(0x19)))) as u32) & 0xffu32);
                fb = ((((0x7i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x31a274; continue; }
                pc = 0x31a223; continue;
            }
            0x31a223 => {
                esi = m.read32(esi.wrapping_add(0x40));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31a1d0; continue; }
                pc = 0x31a22a; continue;
            }
            0x31a22a => {
                eax = edi;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x31a22f => {
                call!(0x3118f0);
                pc = 0x31a234; continue;
            }
            0x31a234 => {
                edx = ((m.read8(esi.wrapping_add(0x18))) as u8 as u32);
                fa = edx;
                fb = eax;
                if (fa != fb) { pc = 0x31a274; continue; }
                pc = 0x31a23c; continue;
            }
            0x31a23c => {
                eax = m.read32(esi.wrapping_add(0x1c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31a274; continue; }
                pc = 0x31a243; continue;
            }
            0x31a243 => {
                eax = m.read32(esi.wrapping_add(0x20));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31a274; continue; }
                pc = 0x31a24a; continue;
            }
            0x31a24a => {
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x24)));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x475d4c)) as f64);
                eax = (((eax & 0xffff00ffu32)) | ((((((if ((((fca == fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x40u32 } else { 0x0u32 }) | (if ((((fca != fca) || (fcb != fcb)))) { 0x4u32 } else { 0x0u32 })) | (if ((((fca < fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x1u32 } else { 0x0u32 })) | 0x2u32)).wrapping_shl((0x8 & 31))));
                fa = (((((((eax) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x44i32)) as u32) & 0xffu32);
                if (((((fa) & (fb))) as u8).count_ones() % 2 == 0) { pc = 0x31a274; continue; }
                pc = 0x31a25c; continue;
            }
            0x31a25c => {
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x28)));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x475cac)) as f64);
                eax = (((eax & 0xffff00ffu32)) | ((((((if ((((fca == fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x40u32 } else { 0x0u32 }) | (if ((((fca != fca) || (fcb != fcb)))) { 0x4u32 } else { 0x0u32 })) | (if ((((fca < fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x1u32 } else { 0x0u32 })) | 0x2u32)).wrapping_shl((0x8 & 31))));
                fa = (((((((eax) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x44i32)) as u32) & 0xffu32);
                if (((((fa) & (fb))) as u8).count_ones() % 2 == 0) { pc = 0x31a274; continue; }
                pc = 0x31a26e; continue;
            }
            0x31a26e => {
                fa = ((((m.read8(esi.wrapping_add(0x19)))) as u32) & 0xffu32);
                fb = ((((0x7i32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x31a22a; continue; }
                pc = 0x31a274; continue;
            }
            0x31a274 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0x1;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0031A300: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31a300<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31a300;
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
        { crate::pose::cover(0x31a300, previous, pc); previous = pc; }
        match pc {
            0x31a300 => {
                push!(esi);
                push!(0);
                push!(0x31a280);
                push!(0xbc3);
                call!(0x3d7b0);
                pc = 0x31a312; continue;
            }
            0x31a312 => {
                esi = m.read32(esp.wrapping_add(0x14));
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(esi);
                call!(0x7a080);
                pc = 0x31a320; continue;
            }
            0x31a320 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(0x18c))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x10);
                m.write8(0x783a60, (((((eax) as u8)) as u32)) as u8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031A330: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31a330<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31a330;
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
        { crate::pose::cover(0x31a330, previous, pc); previous = pc; }
        match pc {
            0x31a330 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                fa = m.read32(esi.wrapping_add(0x18c));
                fb = ((0xffffffffu32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x31a4d1; continue; }
                pc = 0x31a342; continue;
            }
            0x31a342 => {
                eax = m.read32(0x7844a8);
                fa = m.read32(eax.wrapping_add(0x25c));
                fb = ((((0x1000000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) != 0) { pc = 0x31a4d1; continue; }
                pc = 0x31a357; continue;
            }
            0x31a357 => {
                fa = m.read32(esi.wrapping_add(0x4));
                fb = ((((0x52e020i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31a38f; continue; }
                pc = 0x31a360; continue;
            }
            0x31a360 => {
                eax = m.read32(eax.wrapping_add(0x2f18));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31a375; continue; }
                pc = 0x31a36a; continue;
            }
            0x31a36a => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(0x18c))) as u32)) & 0xff);
                m.write8(eax.wrapping_add(0x18), (((((ecx) as u8)) as u32)) as u8);
                pc = 0x31a381; continue;
            }
            0x31a375 => {
                edx = (edx & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(0x18c))) as u32)) & 0xff);
                m.write8(0x783a60, (((((edx) as u8)) as u32)) as u8);
                pc = 0x31a381; continue;
            }
            0x31a381 => {
                push!(esi);
                call!(0x319510);
                pc = 0x31a387; continue;
            }
            0x31a387 => {
                eax = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x4);
                pc = 0x31a38f; continue;
            }
            0x31a38f => {
                ecx = m.read32(esi.wrapping_add(0x4));
                fa = ecx;
                fb = ((((0x52e030i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31a3cf; continue; }
                pc = 0x31a39a; continue;
            }
            0x31a39a => {
                eax = m.read32(eax.wrapping_add(0x2f18));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31a3b8; continue; }
                pc = 0x31a3a4; continue;
            }
            0x31a3a4 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(0x18c))) as u32)) & 0xff);
                push!(esi);
                m.write8(eax.wrapping_add(0x19), (((((ecx) as u8)) as u32)) as u8);
                call!(0x319510);
                pc = 0x31a3b3; continue;
            }
            0x31a3b3 => {
                esp = esp.wrapping_add(0x4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x31a3b8 => {
                edx = (edx & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(0x18c))) as u32)) & 0xff);
                push!(esi);
                m.write8(0x783a61, (((((edx) as u8)) as u32)) as u8);
                call!(0x319510);
                pc = 0x31a3ca; continue;
            }
            0x31a3ca => {
                esp = esp.wrapping_add(0x4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x31a3cf => {
                fa = ecx;
                fb = ((((0x52dff0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31a4aa; continue; }
                pc = 0x31a3db; continue;
            }
            0x31a3db => {
                eax = m.read32(eax.wrapping_add(0x2f18));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31a470; continue; }
                pc = 0x31a3e9; continue;
            }
            0x31a3e9 => {
                ecx = m.read32(esi.wrapping_add(0x18c));
                push!(0x31a110);
                push!(0);
                push!(ecx);
                push!(eax);
                call!(0x31a130);
                pc = 0x31a3fd; continue;
            }
            0x31a3fd => {
                esp = esp.wrapping_add(0x10);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31a431; continue; }
                pc = 0x31a404; continue;
            }
            0x31a404 => {
                edx = m.read32(0x7844a8);
                fa = ((((m.read8(edx.wrapping_add(0x238)))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x31a431; continue; }
                pc = 0x31a413; continue;
            }
            0x31a413 => {
                push!(0x8);
                call!(0x3370a0);
                pc = 0x31a41a; continue;
            }
            0x31a41a => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x238));
                esp = esp.wrapping_add(0x4);
                ecx = (ecx | 0x20);
                m.write32(eax.wrapping_add(0x238), ecx);
                pc = 0x31a431; continue;
            }
            0x31a431 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f18));
                edx = (edx & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(0x18c))) as u32)) & 0xff);
                m.write8(ecx.wrapping_add(0x18), (((((edx) as u8)) as u32)) as u8);
                eax = m.read32(0x78400c);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31a49f; continue; }
                pc = 0x31a44e; continue;
            }
            0x31a44e => {
                ecx = (ecx & 0xffff_ff00) | ((0x2) & 0xff);
                pc = 0x31a450; continue;
            }
            0x31a450 => {
                fa = ((((m.read8(eax.wrapping_add(0x14)))) as u32) & 0xffu32);
                fb = ((((((ecx) as u8))) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x31a45e; continue; }
                pc = 0x31a455; continue;
            }
            0x31a455 => {
                edx = (edx & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(0x18c))) as u32)) & 0xff);
                m.write8(eax.wrapping_add(0x18), (((((edx) as u8)) as u32)) as u8);
                pc = 0x31a45e; continue;
            }
            0x31a45e => {
                eax = m.read32(eax.wrapping_add(0x40));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31a450; continue; }
                pc = 0x31a465; continue;
            }
            0x31a465 => {
                push!(esi);
                call!(0x2fb090);
                pc = 0x31a46b; continue;
            }
            0x31a46b => {
                esp = esp.wrapping_add(0x4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x31a470 => {
                push!(0x1578);
                call!(0x2f79e0);
                pc = 0x31a47a; continue;
            }
            0x31a47a => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(0x18c))) as u32)) & 0xff);
                m.write8(0x783a60, (((((eax) as u8)) as u32)) as u8);
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x25c));
                esp = esp.wrapping_add(0x4);
                ecx = (ecx | 0x100000);
                m.write32(eax.wrapping_add(0x25c), ecx);
                pc = 0x31a49f; continue;
            }
            0x31a49f => {
                push!(esi);
                call!(0x2fb090);
                pc = 0x31a4a5; continue;
            }
            0x31a4a5 => {
                esp = esp.wrapping_add(0x4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x31a4aa => {
                fa = ecx;
                fb = ((((0x52e000i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31a4c0; continue; }
                pc = 0x31a4b2; continue;
            }
            0x31a4b2 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(0x18c))) as u32)) & 0xff);
                m.write8(0x783a60, (((((ecx) as u8)) as u32)) as u8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x31a4c0 => {
                fa = ecx;
                fb = ((((0x52e040i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31a4d1; continue; }
                pc = 0x31a4c8; continue;
            }
            0x31a4c8 => {
                push!(esi);
                call!(0x31a300);
                pc = 0x31a4ce; continue;
            }
            0x31a4ce => {
                esp = esp.wrapping_add(0x4);
                pc = 0x31a4d1; continue;
            }
            0x31a4d1 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031A4E0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31a4e0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31a4e0;
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
        { crate::pose::cover(0x31a4e0, previous, pc); previous = pc; }
        match pc {
            0x31a4e0 => {
                eax = m.read32(0x7844a8);
                m.write32(eax.wrapping_add(0x25c), (m.read32(eax.wrapping_add(0x25c)) & 0xfeffffffu32));
                m.write32(0x784010, 0x10);
                tail_to!(0x746d0); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031A500: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31a500<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31a500;
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
        { crate::pose::cover(0x31a500, previous, pc); previous = pc; }
        match pc {
            0x31a500 => {
                eax = m.read32(esi.wrapping_add(0x4));
                fa = eax;
                fb = ((((0x52e040i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31a536; continue; }
                pc = 0x31a50a; continue;
            }
            0x31a50a => {
                fa = eax;
                fb = ((((0x52e020i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31a536; continue; }
                pc = 0x31a511; continue;
            }
            0x31a511 => {
                fa = eax;
                fb = ((((0x52e030i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31a536; continue; }
                pc = 0x31a518; continue;
            }
            0x31a518 => {
                fa = eax;
                fb = ((((0x52e000i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31a536; continue; }
                pc = 0x31a51f; continue;
            }
            0x31a51f => {
                fa = eax;
                fb = ((((0x52dff0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31a53d; continue; }
                pc = 0x31a526; continue;
            }
            0x31a526 => {
                push!(esi);
                call!(0x31a4e0);
                pc = 0x31a52c; continue;
            }
            0x31a52c => {
                push!(esi);
                call!(0x2fb090);
                pc = 0x31a532; continue;
            }
            0x31a532 => {
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            0x31a536 => {
                push!(esi);
                call!(0x7a7a0);
                pc = 0x31a53c; continue;
            }
            0x31a53c => {
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x31a53d; continue;
            }
            0x31a53d => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031A710: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31a710<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31a710;
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
        { crate::pose::cover(0x31a710, previous, pc); previous = pc; }
        match pc {
            0x31a710 => {
                ecx = m.read32(eax.wrapping_add(0x2c));
                edx = m.read32(ecx.wrapping_add(0x174));
                eax = m.read32(edx);
                fa = eax;
                fb = m.read32(0x52e030);
                if (fa == fb) { pc = 0x31a773; continue; }
                pc = 0x31a723; continue;
            }
            0x31a723 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x11858));
                edx = (((((m.read16(ecx.wrapping_add(0xc)) as i16)) as i32)) as u32);
                edx = ((edx as i32).wrapping_mul((0x8c) as i32)) as u32;
                eax = edx.wrapping_add(eax).wrapping_add(0xdd14);
                push!(eax);
                call!(0x321400);
                pc = 0x31a745; continue;
            }
            0x31a745 => {
                ecx = m.read32(eax);
                m.write32(0x783a60, ecx);
                edx = m.read32(eax.wrapping_add(0x4));
                m.write32(0x783a64, edx);
                ecx = m.read32(eax.wrapping_add(0x8));
                m.write32(0x783a68, ecx);
                edx = m.read32(eax.wrapping_add(0xc));
                m.write32(0x783a6c, edx);
                eax = m.read32(eax.wrapping_add(0x10));
                esp = esp.wrapping_add(0x4);
                m.write32(0x783a70, eax);
                pc = 0x31a773; continue;
            }
            0x31a773 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031A780: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31a780<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31a780;
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
        { crate::pose::cover(0x31a780, previous, pc); previous = pc; }
        match pc {
            0x31a780 => {
                eax = m.read32(0x7844a8);
                fa = m.read32(eax.wrapping_add(0x25c));
                fb = ((((0x1000000i32)) as u32) & 0xffffffffu32);
                push!(0);
                push!(0);
                if ((fa & fb) == 0) { pc = 0x31a7e6; continue; }
                pc = 0x31a795; continue;
            }
            0x31a795 => {
                push!(0xa74);
                call!(0x3d7b0);
                pc = 0x31a79f; continue;
            }
            0x31a79f => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x2);
                call!(0x74a00);
                pc = 0x31a7aa; continue;
            }
            0x31a7aa => {
                esp = esp.wrapping_add(0x10);
                push!(0);
                push!(0);
                push!(0xa75);
                call!(0x3d7b0);
                pc = 0x31a7bb; continue;
            }
            0x31a7bb => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x3);
                call!(0x74a00);
                pc = 0x31a7c6; continue;
            }
            0x31a7c6 => {
                esp = esp.wrapping_add(0x10);
                push!(0);
                push!(0);
                push!(0xa76);
                call!(0x3d7b0);
                pc = 0x31a7d7; continue;
            }
            0x31a7d7 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x1);
                call!(0x74a00);
                pc = 0x31a7e2; continue;
            }
            0x31a7e2 => {
                esp = esp.wrapping_add(0x10);
                break 'run;
            }
            0x31a7e6 => {
                push!(0xa9e);
                call!(0x3d7b0);
                pc = 0x31a7f0; continue;
            }
            0x31a7f0 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0);
                call!(0x74a00);
                pc = 0x31a7fb; continue;
            }
            0x31a7fb => {
                esp = esp.wrapping_add(0x10);
                push!(0);
                push!(0);
                push!(0xab8);
                call!(0x3d7b0);
                pc = 0x31a80c; continue;
            }
            0x31a80c => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x1);
                call!(0x74a00);
                pc = 0x31a817; continue;
            }
            0x31a817 => {
                esp = esp.wrapping_add(0x10);
                push!(0);
                push!(0);
                push!(0xb07);
                call!(0x3d7b0);
                pc = 0x31a828; continue;
            }
            0x31a828 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x2);
                call!(0x74a00);
                pc = 0x31a833; continue;
            }
            0x31a833 => {
                esp = esp.wrapping_add(0x10);
                push!(0);
                push!(0);
                push!(0xa73);
                call!(0x3d7b0);
                pc = 0x31a844; continue;
            }
            0x31a844 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x3);
                call!(0x74a00);
                pc = 0x31a84f; continue;
            }
            0x31a84f => {
                esp = esp.wrapping_add(0x10);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031ACD0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31acd0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31acd0;
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
        { crate::pose::cover(0x31acd0, previous, pc); previous = pc; }
        match pc {
            0x31acd0 => {
                push!(esi);
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0x9ba);
                call!(0x3d7b0);
                pc = 0x31ace3; continue;
            }
            0x31ace3 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79460);
                pc = 0x31acec; continue;
            }
            0x31acec => {
                push!(0x3f800000);
                call!(0x795b0);
                pc = 0x31acf6; continue;
            }
            0x31acf6 => {
                push!(0x8);
                push!(0x1);
                push!(0x1);
                call!(0x78320);
                pc = 0x31ad01; continue;
            }
            0x31ad01 => {
                esp = esp.wrapping_add(0x24);
                esi = 0;
                pc = 0x31ad10; continue;
            }
            0x31ad10 => {
                push!(0x319da0);
                push!(0x31a330);
                push!(0x4094cccd);
                push!(0x4094cccd);
                push!(0);
                push!(esi);
                push!(0);
                call!(0x792b0);
                pc = 0x31ad2e; continue;
            }
            0x31ad2e => {
                push!(0xa1);
                call!(0x716d0);
                pc = 0x31ad38; continue;
            }
            0x31ad38 => {
                esp = esp.wrapping_add(0x20);
                esi = esi.wrapping_add(1);
                fa = esi;
                fb = ((((0x40i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x31ad10; continue; }
                pc = 0x31ad41; continue;
            }
            0x31ad41 => {
                call!(0x78380);
                pc = 0x31ad46; continue;
            }
            0x31ad46 => {
                push!(0x3f800000);
                call!(0x795b0);
                pc = 0x31ad50; continue;
            }
            0x31ad50 => {
                eax = m.read32(esp.wrapping_add(0xc));
                ecx = m.read32(esp.wrapping_add(0x10));
                edx = m.read32(esp.wrapping_add(0x14));
                esp = esp.wrapping_add(0x4);
                push!(0);
                push!(eax);
                push!(0x40400000);
                push!(0x800);
                push!(ecx);
                push!(edx);
                call!(0x3d7b0);
                pc = 0x31ad73; continue;
            }
            0x31ad73 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x31ad7c; continue;
            }
            0x31ad7c => {
                push!(0xa2);
                call!(0x716d0);
                pc = 0x31ad86; continue;
            }
            0x31ad86 => {
                esp = esp.wrapping_add(0x1c);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                tail_to!(0x783d0); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031AF80: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31af80<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31af80;
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
        { crate::pose::cover(0x31af80, previous, pc); previous = pc; }
        match pc {
            0x31af80 => {
                esp = esp.wrapping_sub(0x20);
                push!(ebx);
                push!(ebp);
                push!(esi);
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x34));
                edx = m.read32(edi.wrapping_add(0x4));
                eax = 0;
                fa = edx;
                fb = ((((0x52e000i32)) as u32) & 0xffffffffu32);
                eax = (eax & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                esi = eax;
                eax = m.read32(edi.wrapping_add(0x198));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31afed; continue; }
                pc = 0x31afa5; continue;
            }
            0x31afa5 => {
                fa = m.read32(eax.wrapping_add(0x50));
                fb = ((((0x319da0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31afed; continue; }
                pc = 0x31afae; continue;
            }
            0x31afae => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x25c));
                ebp = 0x1000000;
                fa = ebp;
                fb = eax;
                ebx = 0x1;
                if ((fa & fb) == 0) { pc = 0x31b027; continue; }
                pc = 0x31afc8; continue;
            }
            0x31afc8 => {
                push!(0);
                push!(0x2);
                push!(edi);
                call!(0x74910);
                pc = 0x31afd2; continue;
            }
            0x31afd2 => {
                push!(0);
                push!(0x3);
                push!(edi);
                call!(0x74910);
                pc = 0x31afdc; continue;
            }
            0x31afdc => {
                push!(0);
                push!(ebx);
                push!(edi);
                call!(0x74910);
                pc = 0x31afe5; continue;
            }
            0x31afe5 => {
                esp = esp.wrapping_add(0x24);
                pc = 0x31b072; continue;
            }
            0x31afed => {
                edx = m.read32(0x7844a8);
                eax = m.read32(edx.wrapping_add(0x25c));
                ebp = 0x1000000;
                ebx = 0;
                fa = ebp;
                fb = eax;
                if ((fa & fb) == 0) { pc = 0x31b048; continue; }
                pc = 0x31b004; continue;
            }
            0x31b004 => {
                push!(0x1);
                push!(0x2);
                push!(edi);
                call!(0x74910);
                pc = 0x31b00e; continue;
            }
            0x31b00e => {
                push!(0x1);
                push!(0x3);
                push!(edi);
                call!(0x74910);
                pc = 0x31b018; continue;
            }
            0x31b018 => {
                push!(0x1);
                push!(0x1);
                push!(edi);
                call!(0x74910);
                pc = 0x31b022; continue;
            }
            0x31b022 => {
                esp = esp.wrapping_add(0x24);
                pc = 0x31b072; continue;
            }
            0x31b027 => {
                push!(0xa9e);
                call!(0x3d7b0);
                pc = 0x31b031; continue;
            }
            0x31b031 => {
                push!(eax);
                push!(0);
                push!(edi);
                call!(0x74950);
                pc = 0x31b03a; continue;
            }
            0x31b03a => {
                push!(0);
                push!(0x2);
                push!(edi);
                call!(0x74910);
                pc = 0x31b044; continue;
            }
            0x31b044 => {
                push!(0);
                pc = 0x31b067; continue;
            }
            0x31b048 => {
                push!(0xa97);
                call!(0x3d7b0);
                pc = 0x31b052; continue;
            }
            0x31b052 => {
                push!(eax);
                push!(0);
                push!(edi);
                call!(0x74950);
                pc = 0x31b05b; continue;
            }
            0x31b05b => {
                push!(0x1);
                push!(0x2);
                push!(edi);
                call!(0x74910);
                pc = 0x31b065; continue;
            }
            0x31b065 => {
                push!(0x1);
                pc = 0x31b067; continue;
            }
            0x31b067 => {
                push!(0x3);
                push!(edi);
                call!(0x74910);
                pc = 0x31b06f; continue;
            }
            0x31b06f => {
                esp = esp.wrapping_add(0x28);
                pc = 0x31b072; continue;
            }
            0x31b072 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f18));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31b0bc; continue; }
                pc = 0x31b081; continue;
            }
            0x31b081 => {
                xmm0 = Xmm::scalar(m.read32(ecx));
                push!(0);
                edx = ecx;
                m.write32(esp.wrapping_add(0x28), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(edx.wrapping_add(0x4)));
                eax = ecx;
                push!(0x2);
                ecx = esp.wrapping_add(0x2c);
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x8)));
                push!(edi);
                push!(ecx);
                m.write32(esp.wrapping_add(0x3c), xmm0.lanes[0]);
                call!(0x2fea00);
                pc = 0x31b0b4; continue;
            }
            0x31b0b4 => {
                eax = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x10);
                pc = 0x31b0bc; continue;
            }
            0x31b0bc => {
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31b10c; continue; }
                pc = 0x31b0c0; continue;
            }
            0x31b0c0 => {
                edx = m.read32(eax.wrapping_add(0x11858));
                ecx = (((((m.read16(edx.wrapping_add(0xc)) as i16)) as i32)) as u32);
                ecx = ((ecx as i32).wrapping_mul((0x8c) as i32)) as u32;
                eax = ecx.wrapping_add(eax).wrapping_add(0xdd14);
                push!(eax);
                call!(0x321400);
                pc = 0x31b0dd; continue;
            }
            0x31b0dd => {
                edx = m.read32(0x783a60);
                m.write32(eax, edx);
                ecx = m.read32(0x783a64);
                m.write32(eax.wrapping_add(0x4), ecx);
                edx = m.read32(0x783a68);
                m.write32(eax.wrapping_add(0x8), edx);
                ecx = m.read32(0x783a6c);
                m.write32(eax.wrapping_add(0xc), ecx);
                edx = m.read32(0x783a70);
                esp = esp.wrapping_add(0x4);
                m.write32(eax.wrapping_add(0x10), edx);
                pc = 0x31b10c; continue;
            }
            0x31b10c => {
                push!(0x65);
                push!(edi);
                call!(0x75580);
                pc = 0x31b114; continue;
            }
            0x31b114 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31b149; continue; }
                pc = 0x31b11b; continue;
            }
            0x31b11b => {
                push!(0x65);
                push!(edi);
                call!(0x71940);
                pc = 0x31b123; continue;
            }
            0x31b123 => {
                push!(0x66);
                push!(edi);
                esi = eax;
                call!(0x71940);
                pc = 0x31b12d; continue;
            }
            0x31b12d => {
                esp = esp.wrapping_add(0x10);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31b149; continue; }
                pc = 0x31b134; continue;
            }
            0x31b134 => {
                ecx = m.read32(esi.wrapping_add(0x48));
                fa = m.read32(ecx);
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                ecx = 0x31a580;
                if (fas > fbs) { pc = 0x31b146; continue; }
                pc = 0x31b141; continue;
            }
            0x31b141 => {
                ecx = 0x31a8c0;
                pc = 0x31b146; continue;
            }
            0x31b146 => {
                m.write32(eax.wrapping_add(0x38), ecx);
                pc = 0x31b149; continue;
            }
            0x31b149 => {
                push!(edi);
                call!(0x73de0);
                pc = 0x31b14f; continue;
            }
            0x31b14f => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31b47e; continue; }
                pc = 0x31b15a; continue;
            }
            0x31b15a => {
                edx = m.read32(0x7ba9e0);
                edx = ((edx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                eax = m.read32(edx.wrapping_add(0x7ba26c));
                push!(eax);
                call!(0x77470);
                pc = 0x31b172; continue;
            }
            0x31b172 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x31b19d; continue; }
                pc = 0x31b180; continue;
            }
            0x31b180 => {
                eax = m.read32(0x7844a8);
                fa = m.read32(eax.wrapping_add(0x25c));
                fb = ebp;
                if ((fa & fb) != 0) { pc = 0x31b19d; continue; }
                pc = 0x31b18d; continue;
            }
            0x31b18d => {
                fa = m.read32(eax.wrapping_add(0x254));
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31b19d; continue; }
                pc = 0x31b196; continue;
            }
            0x31b196 => {
                esi = edi;
                call!(0x31a500);
                pc = 0x31b19d; continue;
            }
            0x31b19d => {
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31b47e; continue; }
                pc = 0x31b1a5; continue;
            }
            0x31b1a5 => {
                edx = m.read32(0x7ba9e0);
                ecx = m.read32(0x7844a8);
                edx = ((edx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                fa = m.read32(ecx.wrapping_add(0x25c));
                fb = ebp;
                eax = m.read32(edx.wrapping_add(0x7ba26c));
                push!(eax);
                if ((fa & fb) == 0) { pc = 0x31b3bf; continue; }
                pc = 0x31b1ca; continue;
            }
            0x31b1ca => {
                call!(0x77470);
                pc = 0x31b1cf; continue;
            }
            0x31b1cf => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x31b1fd; continue; }
                pc = 0x31b1dd; continue;
            }
            0x31b1dd => {
                push!(0x1577);
                call!(0x2f79e0);
                pc = 0x31b1e7; continue;
            }
            0x31b1e7 => {
                call!(0x73c80);
                pc = 0x31b1ec; continue;
            }
            0x31b1ec => {
                push!(edi);
                call!(0x31a4e0);
                pc = 0x31b1f2; continue;
            }
            0x31b1f2 => {
                esp = esp.wrapping_add(0x8);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x20);
                break 'run;
            }
            0x31b1fd => {
                eax = m.read32(edi.wrapping_add(0x18c));
                ebx = 0x40;
                fa = eax;
                fb = ebx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x31b47e; continue; }
                pc = 0x31b210; continue;
            }
            0x31b210 => {
                ecx = m.read32(0x7ba9e0);
                ecx = ((ecx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                edx = m.read32(ecx.wrapping_add(0x7ba26c));
                push!(edx);
                call!(0x77470);
                pc = 0x31b228; continue;
            }
            0x31b228 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((((ecx) as u8))) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x31b26b; continue; }
                pc = 0x31b235; continue;
            }
            0x31b235 => {
                push!(0x157a);
                call!(0x2f79e0);
                pc = 0x31b23f; continue;
            }
            0x31b23f => {
                eax = ((m.read8(0x783a60)) as u8 as u32);
                ecx = m.read32(edi.wrapping_add(0x18c));
                edx = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x783a80));
                push!(edi);
                m.write32(ecx.wrapping_mul(0x4).wrapping_add(0x783a80), edx);
                call!(0x31a4e0);
                pc = 0x31b260; continue;
            }
            0x31b260 => {
                esp = esp.wrapping_add(0x8);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x20);
                break 'run;
            }
            0x31b26b => {
                eax = m.read32(0x7ba9e0);
                eax = ((eax as i32).wrapping_mul((0x1e0) as i32)) as u32;
                ecx = m.read32(eax.wrapping_add(0x7ba26c));
                push!(ecx);
                call!(0x77470);
                pc = 0x31b282; continue;
            }
            0x31b282 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (((((((((((((fa)) as u8)) as i32).wrapping_sub((((((fb)) as u8)) as i32)))) as u8)) as i32).wrapping_shr((0x7 & 31))) == 0i32)) { pc = 0x31b47e; continue; }
                pc = 0x31b293; continue;
            }
            0x31b293 => {
                eax = ((m.read8(0x783a60)) as u8 as u32);
                ecx = m.read32(edi.wrapping_add(0x18c));
                fa = ecx;
                fb = eax;
                if (fa == fb) { pc = 0x31b47e; continue; }
                pc = 0x31b2a8; continue;
            }
            0x31b2a8 => {
                edx = eax;
                if (fa <= fb) { edx = ecx; }
                edx = m.read32(edx.wrapping_mul(0x4).wrapping_add(0x783a80));
                m.write32(esp.wrapping_add(0x18), edx);
                edx = ecx;
                if (fa <= fb) { edx = eax; }
                edx = m.read32(edx.wrapping_mul(0x4).wrapping_add(0x783a80));
                m.write32(esp.wrapping_add(0x1c), edx);
                edx = eax;
                eax = eax.wrapping_shr((((((((0x3i32)) as u32) & 0x1fu32))) as u32 & 31));
                esi = eax;
                eax = ecx;
                edx = (edx & 0x7);
                eax = (eax & 0x7);
                ecx = ecx.wrapping_shr((((((((0x3i32)) as u32) & 0x1fu32))) as u32 & 31));
                fa = edx;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x31b2e4; continue; }
                pc = 0x31b2de; continue;
            }
            0x31b2de => {
                ebx = eax;
                eax = edx;
                edx = ebx;
                pc = 0x31b2e4; continue;
            }
            0x31b2e4 => {
                fa = esi;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x31b2ee; continue; }
                pc = 0x31b2e8; continue;
            }
            0x31b2e8 => {
                ebx = ecx;
                ecx = esi;
                esi = ebx;
                pc = 0x31b2ee; continue;
            }
            0x31b2ee => {
                eax = eax.wrapping_sub(edx);
                ecx = ecx.wrapping_sub(esi);
                eax = eax.wrapping_add(1);
                ecx = ecx.wrapping_add(1);
                ecx = ((ecx as i32).wrapping_mul((eax) as i32)) as u32;
                ebx = 0;
                ebp = 0;
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x10), eax);
                m.write32(esp.wrapping_add(0x20), ecx);
                esi = edx.wrapping_add(esi.wrapping_mul(0x8));
                if (fas <= fbs) { pc = 0x31b377; continue; }
                pc = 0x31b30a; continue;
            }
            0x31b30a => {
                ecx = ecx.wrapping_sub(1);
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                m.write32(esp.wrapping_add(0x14), xmm0.lanes[0]);
                edi = esi.wrapping_mul(0x4);
                pc = 0x31b322; continue;
            }
            0x31b31e => {
                eax = m.read32(esp.wrapping_add(0x10));
                pc = 0x31b322; continue;
            }
            0x31b322 => {
                fa = ebx;
                fb = eax;
                if (fa != fb) { pc = 0x31b33d; continue; }
                pc = 0x31b326; continue;
            }
            0x31b326 => {
                ecx = eax.wrapping_mul(0x4);
                edi = edi.wrapping_sub(ecx);
                ecx = 0x8;
                ecx = ecx.wrapping_sub(eax);
                ebx = 0;
                esi = esi.wrapping_add(ecx);
                edi = edi.wrapping_add(0x20);
                pc = 0x31b33d; continue;
            }
            0x31b33d => {
                edx = m.read32(esp.wrapping_add(0x18));
                eax = m.read32(esp.wrapping_add(0x1c));
                push!(ecx);
                xmm0.lanes[0] = (((ebp) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::divss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x18)))).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                push!(edx);
                push!(eax);
                call!(0x154c50);
                pc = 0x31b35c; continue;
            }
            0x31b35c => {
                esp = esp.wrapping_add(0xc);
                m.write32(edi.wrapping_add(0x783a80), eax);
                eax = m.read32(esp.wrapping_add(0x20));
                esi = esi.wrapping_add(1);
                edi = edi.wrapping_add(0x4);
                ebx = ebx.wrapping_add(1);
                ebp = ebp.wrapping_add(1);
                fa = ebp;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x31b31e; continue; }
                pc = 0x31b373; continue;
            }
            0x31b373 => {
                edi = m.read32(esp.wrapping_add(0x34));
                pc = 0x31b377; continue;
            }
            0x31b377 => {
                ecx = m.read32(0x7844a8);
                fa = ((((m.read8(ecx.wrapping_add(0x238)))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x31b3a4; continue; }
                pc = 0x31b386; continue;
            }
            0x31b386 => {
                push!(0xb);
                call!(0x3370a0);
                pc = 0x31b38d; continue;
            }
            0x31b38d => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x238));
                esp = esp.wrapping_add(0x4);
                ecx = (ecx | 0x20);
                m.write32(eax.wrapping_add(0x238), ecx);
                pc = 0x31b3a4; continue;
            }
            0x31b3a4 => {
                push!(edi);
                call!(0x31a4e0);
                pc = 0x31b3aa; continue;
            }
            0x31b3aa => {
                push!(0x1626);
                call!(0x2f79e0);
                pc = 0x31b3b4; continue;
            }
            0x31b3b4 => {
                esp = esp.wrapping_add(0x8);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x20);
                break 'run;
            }
            0x31b3bf => {
                call!(0x77470);
                pc = 0x31b3c4; continue;
            }
            0x31b3c4 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (((((((((((((fa)) as u8)) as i32).wrapping_sub((((((fb)) as u8)) as i32)))) as u8)) as i32).wrapping_shr((0x7 & 31))) == 0i32)) { pc = 0x31b415; continue; }
                pc = 0x31b3d1; continue;
            }
            0x31b3d1 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(edi.wrapping_add(0x18c))) as u32)) & 0xff);
                eax = m.read32(0x7844a8);
                m.write8(0x783a60, (((((ecx) as u8)) as u32)) as u8);
                ebx = m.read32(eax.wrapping_add(0x25c));
                ebx = (ebx | ebp);
                push!(edi);
                m.write32(eax.wrapping_add(0x25c), ebx);
                call!(0x746d0);
                pc = 0x31b3f6; continue;
            }
            0x31b3f6 => {
                push!(0x1578);
                m.write32(0x784010, 0xf);
                call!(0x2f79e0);
                pc = 0x31b40a; continue;
            }
            0x31b40a => {
                esp = esp.wrapping_add(0x8);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x20);
                break 'run;
            }
            0x31b415 => {
                edx = m.read32(0x7ba9e0);
                edx = ((edx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                eax = m.read32(edx.wrapping_add(0x7ba26c));
                push!(eax);
                call!(0x77470);
                pc = 0x31b42d; continue;
            }
            0x31b42d => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x40i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x31b47e; continue; }
                pc = 0x31b43b; continue;
            }
            0x31b43b => {
                ecx = m.read32(edi.wrapping_add(0x18c));
                edx = m.read32(ecx.wrapping_mul(0x4).wrapping_add(0x783a80));
                push!(0x783b80);
                push!(edx);
                call!(0x154bc0);
                pc = 0x31b453; continue;
            }
            0x31b453 => {
                push!(0x762f84);
                push!(0x783b80);
                call!(0x154990);
                pc = 0x31b462; continue;
            }
            0x31b462 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(edi.wrapping_add(0x18c))) as u32)) & 0xff);
                m.write8(0x783a60, (((((eax) as u8)) as u32)) as u8);
                ecx = m.read32(edi.wrapping_add(0x2c));
                push!(0x52dfe0);
                push!(ecx);
                call!(0x79b70);
                pc = 0x31b47b; continue;
            }
            0x31b47b => {
                esp = esp.wrapping_add(0x18);
                pc = 0x31b47e; continue;
            }
            0x31b47e => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x20);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0031B490: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31b490<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31b490;
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
        { crate::pose::cover(0x31b490, previous, pc); previous = pc; }
        match pc {
            0x31b490 => {
                push!(esi);
                esi = m.read32(0x78400c);
                push!(edi);
                edi = m.read32(esp.wrapping_add(0xc));
                push!(0x65);
                push!(edi);
                call!(0x75580);
                pc = 0x31b4a4; continue;
            }
            0x31b4a4 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31b4db; continue; }
                pc = 0x31b4ab; continue;
            }
            0x31b4ab => {
                push!(ebx);
                push!(0x65);
                push!(edi);
                call!(0x71940);
                pc = 0x31b4b4; continue;
            }
            0x31b4b4 => {
                push!(0x66);
                push!(edi);
                ebx = eax;
                call!(0x71940);
                pc = 0x31b4be; continue;
            }
            0x31b4be => {
                esp = esp.wrapping_add(0x10);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31b4da; continue; }
                pc = 0x31b4c5; continue;
            }
            0x31b4c5 => {
                ecx = m.read32(ebx.wrapping_add(0x48));
                fa = m.read32(ecx);
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                ecx = 0x31a580;
                if (fas > fbs) { pc = 0x31b4d7; continue; }
                pc = 0x31b4d2; continue;
            }
            0x31b4d2 => {
                ecx = 0x31a8c0;
                pc = 0x31b4d7; continue;
            }
            0x31b4d7 => {
                m.write32(eax.wrapping_add(0x38), ecx);
                pc = 0x31b4da; continue;
            }
            0x31b4da => {
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x31b4db; continue;
            }
            0x31b4db => {
                edx = m.read32(0x7844a8);
                eax = m.read32(edx.wrapping_add(0x2f18));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31b51a; continue; }
                pc = 0x31b4eb; continue;
            }
            0x31b4eb => {
                ecx = m.read32(0x783a60);
                eax = eax.wrapping_add(0x18);
                m.write32(eax, ecx);
                edx = m.read32(0x783a64);
                m.write32(eax.wrapping_add(0x4), edx);
                ecx = m.read32(0x783a68);
                m.write32(eax.wrapping_add(0x8), ecx);
                edx = m.read32(0x783a6c);
                m.write32(eax.wrapping_add(0xc), edx);
                ecx = m.read32(0x783a70);
                m.write32(eax.wrapping_add(0x10), ecx);
                pc = 0x31b51a; continue;
            }
            0x31b51a => {
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31b55b; continue; }
                pc = 0x31b51e; continue;
            }
            0x31b51e => {
                eax = (eax & 0xffff_ff00) | ((0x2) & 0xff);
                pc = 0x31b520; continue;
            }
            0x31b520 => {
                fa = ((((m.read8(esi.wrapping_add(0x14)))) as u32) & 0xffu32);
                fb = ((((((eax) as u8))) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x31b554; continue; }
                pc = 0x31b525; continue;
            }
            0x31b525 => {
                ecx = m.read32(0x783a60);
                edx = esi.wrapping_add(0x18);
                m.write32(edx, ecx);
                ecx = m.read32(0x783a64);
                m.write32(edx.wrapping_add(0x4), ecx);
                ecx = m.read32(0x783a68);
                m.write32(edx.wrapping_add(0x8), ecx);
                ecx = m.read32(0x783a6c);
                m.write32(edx.wrapping_add(0xc), ecx);
                ecx = m.read32(0x783a70);
                m.write32(edx.wrapping_add(0x10), ecx);
                pc = 0x31b554; continue;
            }
            0x31b554 => {
                esi = m.read32(esi.wrapping_add(0x40));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31b520; continue; }
                pc = 0x31b55b; continue;
            }
            0x31b55b => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}
}
pub use unformatted::*;
