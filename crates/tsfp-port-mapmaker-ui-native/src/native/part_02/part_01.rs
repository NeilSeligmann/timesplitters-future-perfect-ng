use super::*; // portreg: replaces=tsfp-ports/mapmaker_ui_lane
#[rustfmt::skip] mod unformatted { use super::*;
/// 0x0030D410: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f30d410<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x30d410;
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
        { crate::pose::cover(0x30d410, previous, pc); previous = pc; }
        match pc {
            0x30d410 => {
                ecx = m.read32(esp.wrapping_add(0x4));
                xmm1 = Xmm::scalar(m.read32(ecx));
                xmm0 = Xmm::default();
                edx = m.read32(esp.wrapping_add(0x8));
                eax = 0;
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                push!(esi);
                esi = m.read32(0x4e8cf4);
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30d444; continue; }
                pc = 0x30d42d; continue;
            }
            0x30d42d => {
                xmm1.lanes[0] = (((esi) as i32) as f32).to_bits();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(ecx)) as f64);
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30d444; continue; }
                pc = 0x30d436; continue;
            }
            0x30d436 => {
                xmm2 = Xmm::scalar(m.read32(edx));
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30d444; continue; }
                pc = 0x30d43f; continue;
            }
            0x30d43f => {
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm2.lanes[0]) as f64;
                if ((fca >= fcb)) { pc = 0x30d4bc; continue; }
                pc = 0x30d444; continue;
            }
            0x30d444 => {
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(ecx)) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30d46d; continue; }
                pc = 0x30d449; continue;
            }
            0x30d449 => {
                xmm2 = Xmm::scalar(m.read32(edx));
                xmm1.lanes[0] = (((esi) as i32) as f32).to_bits();
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30d46d; continue; }
                pc = 0x30d456; continue;
            }
            0x30d456 => {
                m.write32(ecx, xmm0.lanes[0]);
                xmm0.lanes[0] = (((m.read32(0x4e8cf4)) as i32) as f32).to_bits();
                m.write32(edx, xmm0.lanes[0]);
                eax = 0x1;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x30d46d => {
                xmm1 = Xmm::scalar(m.read32(ecx));
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30d498; continue; }
                pc = 0x30d476; continue;
            }
            0x30d476 => {
                xmm1.lanes[0] = (((esi) as i32) as f32).to_bits();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(ecx)) as f64);
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30d498; continue; }
                pc = 0x30d47f; continue;
            }
            0x30d47f => {
                xmm2 = Xmm::scalar(m.read32(edx));
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30d48d; continue; }
                pc = 0x30d488; continue;
            }
            0x30d488 => {
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm2.lanes[0]) as f64;
                if ((fca >= fcb)) { pc = 0x30d498; continue; }
                pc = 0x30d48d; continue;
            }
            0x30d48d => {
                m.write32(edx, xmm1.lanes[0]);
                eax = 0x1;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x30d498 => {
                xmm1 = Xmm::scalar(m.read32(edx));
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30d4c1; continue; }
                pc = 0x30d4a1; continue;
            }
            0x30d4a1 => {
                xmm1.lanes[0] = (((esi) as i32) as f32).to_bits();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(edx)) as f64);
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30d4c1; continue; }
                pc = 0x30d4aa; continue;
            }
            0x30d4aa => {
                xmm2 = Xmm::scalar(m.read32(ecx));
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30d4b8; continue; }
                pc = 0x30d4b3; continue;
            }
            0x30d4b3 => {
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm2.lanes[0]) as f64;
                if ((fca >= fcb)) { pc = 0x30d4c1; continue; }
                pc = 0x30d4b8; continue;
            }
            0x30d4b8 => {
                m.write32(ecx, xmm0.lanes[0]);
                pc = 0x30d4bc; continue;
            }
            0x30d4bc => {
                eax = 0x1;
                pc = 0x30d4c1; continue;
            }
            0x30d4c1 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}
/// 0x0030D4D0: compile-only, below Tier 0; fixture debt, unavailable through run().
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f30d4d0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x30d4d0;
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
        { crate::pose::cover(0x30d4d0, previous, pc); previous = pc; }
        match pc {
            0x30d4d0 => {
                esp = esp.wrapping_sub(0x68);
                ecx = m.read32(eax);
                xmm2.lanes[0] = (((m.read32(eax.wrapping_add(0x8))) as i32) as f32).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x47d654))).to_bits();
                push!(ebx);
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0x7c));
                push!(esi);
                esi = ebp;
                esi = (esi & 0xffffff1eu32);
                esi = (esi | 0x1e);
                fa = ecx;
                fb = ((((0x35i32)) as u32) & 0xffffffffu32);
                push!(edi);
                m.write32(esp.wrapping_add(0x1c), 0x1);
                m.write32(esp.wrapping_add(0x18), xmm2.lanes[0]);
                if (fa == fb) { pc = 0x30d55f; continue; }
                pc = 0x30d508; continue;
            }
            0x30d508 => {
                fa = ecx;
                fb = ((((0x36i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30d55f; continue; }
                pc = 0x30d50d; continue;
            }
            0x30d50d => {
                fa = ecx;
                fb = ((((0x37i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30d55f; continue; }
                pc = 0x30d512; continue;
            }
            0x30d512 => {
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x18)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475c84))).to_bits();
                xmm3 = Xmm::scalar(m.read32(0x475d0c));
                edi = m.read32(esp.wrapping_add(0x80));
                ebx = m.read32(esp.wrapping_add(0x7c));
                xmm1 = xmm2;
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x14), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x84), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x18), xmm1.lanes[0]);
                pc = 0x30d600; continue;
            }
            0x30d55f => {
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x1c)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475c8c))).to_bits();
                m.write32(esp.wrapping_add(0x84), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x1c)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475c84))).to_bits();
                eax = esp.wrapping_add(0x10);
                push!(eax);
                ecx = esp.wrapping_add(0x88);
                push!(ecx);
                m.write32(esp.wrapping_add(0x18), xmm0.lanes[0]);
                call!(0x319440);
                pc = 0x30d59a; continue;
            }
            0x30d59a => {
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x20)));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x8c)));
                xmm1 = Xmm::scalar(m.read32(0x475d0c));
                edi = m.read32(esp.wrapping_add(0x88));
                ebx = m.read32(esp.wrapping_add(0x84));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x1c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x18)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x1c)));
                esp = esp.wrapping_add(0x8);
                m.write32(esp.wrapping_add(0x84), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x18), xmm1.lanes[0]);
                pc = 0x30d600; continue;
            }
            0x30d5f5 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x84)));
                edi = edi;
                pc = 0x30d600; continue;
            }
            0x30d600 => {
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x475d0c))).to_bits();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x10), xmm1.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30d621; continue; }
                pc = 0x30d613; continue;
            }
            0x30d613 => {
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x1c), 0);
                pc = 0x30d621; continue;
            }
            0x30d621 => {
                xmm0 = Xmm::default();
                xmm1 = Xmm::scalar(m.read32(0x478674));
                edx = esp.wrapping_add(0x38);
                push!(edx);
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x38), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x2c), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x34), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x28), xmm0.lanes[0]);
                call!(0x153240);
                pc = 0x30d65a; continue;
            }
            0x30d65a => {
                eax = m.read32(esp.wrapping_add(0x1c));
                push!(eax);
                ecx = esp.wrapping_add(0x40);
                push!(ecx);
                call!(0x153440);
                pc = 0x30d669; continue;
            }
            0x30d669 => {
                edx = esp.wrapping_add(0x38);
                push!(edx);
                eax = esp.wrapping_add(0x48);
                push!(eax);
                call!(0x151e80);
                pc = 0x30d678; continue;
            }
            0x30d678 => {
                ecx = esp.wrapping_add(0x4c);
                push!(ecx);
                call!(0x153240);
                pc = 0x30d682; continue;
            }
            0x30d682 => {
                edx = m.read32(esp.wrapping_add(0x28));
                push!(edx);
                eax = esp.wrapping_add(0x54);
                push!(eax);
                call!(0x153440);
                pc = 0x30d691; continue;
            }
            0x30d691 => {
                ecx = esp.wrapping_add(0x40);
                push!(ecx);
                edx = esp.wrapping_add(0x5c);
                push!(edx);
                call!(0x151e80);
                pc = 0x30d6a0; continue;
            }
            0x30d6a0 => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x48))) as f64));
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(esp.wrapping_add(0xa4))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x5c)));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x54)));
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0xa8)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0xa4)))).to_bits();
                m.write32(esp.wrapping_add(0x48), (fpu.st(0) as f32).to_bits()); fpu.pop();
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x50))) as f64));
                ecx = m.read32(esp.wrapping_add(0x48));
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(esp.wrapping_add(0xa8))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(esi);
                push!(esi);
                push!(ebp);
                push!(ebp);
                m.write32(esp.wrapping_add(0x60), (fpu.st(0) as f32).to_bits()); fpu.pop();
                eax = m.read32(esp.wrapping_add(0x60));
                push!(eax);
                push!(ecx);
                m.write32(esp.wrapping_add(0x74), xmm1.lanes[0]);
                edx = m.read32(esp.wrapping_add(0x74));
                push!(edx);
                m.write32(esp.wrapping_add(0x70), xmm0.lanes[0]);
                eax = m.read32(esp.wrapping_add(0x70));
                xmm0 = Xmm::default();
                push!(eax);
                push!(edi);
                push!(ebx);
                push!(edi);
                push!(ebx);
                m.write32(esp.wrapping_add(0x88), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x7c), xmm0.lanes[0]);
                call!(0x308c00);
                pc = 0x30d71b; continue;
            }
            0x30d71b => {
                eax = m.read32(esp.wrapping_add(0x74));
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x68)));
                esp = esp.wrapping_add(0x58);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0x18), xmm1.lanes[0]);
                if (fa != fb) { pc = 0x30d5f5; continue; }
                pc = 0x30d736; continue;
            }
            0x30d736 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x478910))).to_bits();
                push!(ecx);
                m.write32(esp, xmm0.lanes[0]);
                push!(0x3c75c28f);
                push!(0x3f99999a);
                push!(0xbc75c28fu32);
                push!(0);
                push!(edi);
                push!(ebx);
                esi = 0x7f7f7f80;
                call!(0x30b7c0);
                pc = 0x30d767; continue;
            }
            0x30d767 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0xa0)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x478910))).to_bits();
                esp = esp.wrapping_add(0x18);
                m.write32(esp, xmm0.lanes[0]);
                push!(0x3c75c28f);
                push!(0x3f99999a);
                push!(0xbc75c28fu32);
                push!(0);
                push!(edi);
                push!(ebx);
                call!(0x30b7c0);
                pc = 0x30d798; continue;
            }
            0x30d798 => {
                esp = esp.wrapping_add(0x1c);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x68);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0030D7B0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f30d7b0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x30d7b0;
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
        { crate::pose::cover(0x30d7b0, previous, pc); previous = pc; }
        match pc {
            0x30d7b0 => {
                esp = esp.wrapping_sub(0x24);
                xmm1 = Xmm::scalar(m.read32(0x475cd4));
                push!(ebx);
                push!(ebp);
                push!(esi);
                push!(edi);
                edi = eax;
                eax = m.read32(esp.wrapping_add(0x38));
                ebx = m.read32(eax);
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x254));
                ebp = ebx;
                ebp = ((ebp as i32).wrapping_mul((0xc4) as i32)) as u32;
                ebp = ebp.wrapping_add(0x524f58);
                fa = ecx;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x14), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x10), xh[0].lanes[0]);
                if (fa != fb) { pc = 0x30d80a; continue; }
                pc = 0x30d7f9; continue;
            }
            0x30d7f9 => {
                fa = m.read32(eax.wrapping_add(0x2f34));
                fb = ((0xffffffffu32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0x28), 0);
                if (fa == fb) { pc = 0x30d812; continue; }
                pc = 0x30d80a; continue;
            }
            0x30d80a => {
                m.write32(esp.wrapping_add(0x28), 0x1);
                pc = 0x30d812; continue;
            }
            0x30d812 => {
                xmm1.lanes[0] = (((m.read32(0x4e8cf0)) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x49f7b8))).to_bits();
                xmm2 = Xmm::scalar(m.read32(0x783f2c));
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm2.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30d9a2; continue; }
                pc = 0x30d833; continue;
            }
            0x30d833 => {
                push!(0x3);
                call!(0x80f20);
                pc = 0x30d83a; continue;
            }
            0x30d83a => {
                xmm1 = Xmm::scalar(m.read32(0x783f2c));
                xmm3 = Xmm::scalar(m.read32(0x475d40));
                xmm2 = Xmm::scalar(m.read32(0x783f28));
                xmm4.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xmm0 = xmm1;
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x34), xmm0.lanes[0]);
                xmm0 = xmm2;
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = Xmm::scalar(m.read32(0x475d48));
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x18)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x20), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                xmm0.lanes[0] = (((m.read32(0x4e8cf0)) as i32) as f32).to_bits();
                m.write32(esp.wrapping_add(0x1c), xmm0.lanes[0]);
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x4785d4))).to_bits();
                esp = esp.wrapping_add(0x4);
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                esi = eax;
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30d92a; continue; }
                pc = 0x30d8ea; continue;
            }
            0x30d8ea => {
                fpu.push((f32::from_bits(m.read32(0x783f2c)) as f64));
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(esp.wrapping_add(0x24))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x47d3ac)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x18))) as f64));
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x49f7b8)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(esp.wrapping_add(0x24))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x47d3ac)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(1) / fpu.st(0), fpu.control); fpu.stack[((fpu.top + 1) & 7) as usize] = value; } fpu.pop();
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4765f0)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                call!(0x3c848c);
                pc = 0x30d91b; continue;
            }
            0x30d91b => {
                ecx = (ecx & 0xffff_ff00) | ((0x80) & 0xff);
                ecx = (ecx & 0xffff_ff00) | ((((((((ecx) as u8)) as i32).wrapping_sub(((((eax) as u8)) as i32))) as u32)) & 0xff);
                edx = ((((ecx) as u8)) as u8 as u32);
                edi = (edi & 0xffffff00u32);
                edi = (edi | edx);
                pc = 0x30d92a; continue;
            }
            0x30d92a => {
                push!(esi);
                call!(0x1ed50);
                pc = 0x30d930; continue;
            }
            0x30d930 => {
                esi = ((esi as i32).wrapping_mul((0x34) as i32)) as u32;
                eax = m.read32(0x4b85c8);
                ecx = m.read32(eax);
                edx = ((m.read16(esi.wrapping_add(ecx).wrapping_add(0x2a))) as u16 as u32);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x30)))).to_bits();
                eax = esi.wrapping_add(ecx);
                eax = ((m.read16(eax.wrapping_add(0x26))) as u16 as u32);
                ecx = m.read32(esp.wrapping_add(0x24));
                esp = esp.wrapping_add(0x4);
                push!(edi);
                edx = edx.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                push!(edx);
                edx = m.read32(esp.wrapping_add(0x24));
                eax = eax.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                push!(eax);
                push!(0);
                push!(0);
                esp = esp.wrapping_sub(0x8);
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x38)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x4c)))).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                push!(ecx);
                push!(edx);
                call!(0x1b7b0);
                pc = 0x30d98b; continue;
            }
            0x30d98b => {
                xmm2 = Xmm::scalar(m.read32(0x783f2c));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x38)));
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x34)));
                esp = esp.wrapping_add(0x24);
                pc = 0x30d9a2; continue;
            }
            0x30d9a2 => {
                xmm1.lanes[0] = (((m.read32(0x4e8cf0)) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x4785d4))).to_bits();
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30dd2d; continue; }
                pc = 0x30d9bb; continue;
            }
            0x30d9bb => {
                xmm4 = Xmm::scalar(m.read32(0x475d40));
                xmm3 = Xmm::scalar(m.read32(0x783f28));
                xmm1 = xmm2;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x24), xmm1.lanes[0]);
                xmm1 = xmm3;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x20), xmm1.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(0x475d48));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x1c), xmm0.lanes[0]);
                xmm0 = xh[0];
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm1.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                push!(ebx);
                m.write32(esp.wrapping_add(0x1c), xmm0.lanes[0]);
                call!(0x315d20);
                pc = 0x30da3a; continue;
            }
            0x30da3a => {
                xmm0.lanes[0] = (((m.read32(0x4e8cf0)) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x49f7b8))).to_bits();
                xmm1 = Xmm::scalar(m.read32(0x783f2c));
                esp = esp.wrapping_add(0x4);
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                esi = eax;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30da72; continue; }
                pc = 0x30da5c; continue;
            }
            0x30da5c => {
                eax = edi;
                eax = (eax & 0xff);
                ecx = 0x80;
                ecx = ecx.wrapping_sub(eax);
                edi = (edi & 0xffffff00u32);
                edi = (edi | ecx);
                pc = 0x30da72; continue;
            }
            0x30da72 => {
                fa = ebx;
                fb = ((((0x45i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x30db53; continue; }
                pc = 0x30da7b; continue;
            }
            0x30da7b => {
                xmm2 = Xmm::scalar(m.read32(0x475d4c));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xmm3.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                m.write32(esp.wrapping_add(0x1c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x10)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm2 = Xmm::scalar(m.read32(0x783f28));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x18), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x475cd4));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                push!(0x31);
                m.write32(esp.wrapping_add(0x28), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x24), xmm2.lanes[0]);
                call!(0x81c10);
                pc = 0x30dafe; continue;
            }
            0x30dafe => {
                push!(eax);
                call!(0x1ed50);
                pc = 0x30db04; continue;
            }
            0x30db04 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x20)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x28)))).to_bits();
                edx = m.read32(esp.wrapping_add(0x20));
                eax = m.read32(esp.wrapping_add(0x24));
                esp = esp.wrapping_add(0x8);
                push!(edi);
                push!(0x100);
                push!(0x100);
                push!(0);
                push!(0);
                esp = esp.wrapping_sub(0x8);
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x38)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x40)))).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                push!(edx);
                push!(eax);
                call!(0x1b7b0);
                pc = 0x30db4b; continue;
            }
            0x30db4b => {
                esp = esp.wrapping_add(0x24);
                pc = 0x30dbf9; continue;
            }
            0x30db53 => {
                push!(esi);
                call!(0x1ed50);
                pc = 0x30db59; continue;
            }
            0x30db59 => {
                esi = ((esi as i32).wrapping_mul((0x34) as i32)) as u32;
                ecx = m.read32(0x4b85c8);
                edx = m.read32(ecx);
                eax = ((m.read16(esi.wrapping_add(edx).wrapping_add(0x26))) as u16 as u32);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x1c)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x24)))).to_bits();
                ecx = m.read32(esp.wrapping_add(0x20));
                esp = esp.wrapping_add(0x4);
                push!(edi);
                eax = eax.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                push!(eax);
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x24));
                push!(0);
                push!(0);
                esp = esp.wrapping_sub(0x8);
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x38)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x40)))).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                push!(eax);
                push!(ecx);
                call!(0x1b7b0);
                pc = 0x30dbab; continue;
            }
            0x30dbab => {
                esp = esp.wrapping_add(0x24);
                fa = ebx;
                fb = ((((0x35i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30dbcc; continue; }
                pc = 0x30dbb3; continue;
            }
            0x30dbb3 => {
                fa = ebx;
                fb = ((((0x36i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30dbcc; continue; }
                pc = 0x30dbb8; continue;
            }
            0x30dbb8 => {
                fa = ebx;
                fb = ((((0x32i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30dbcc; continue; }
                pc = 0x30dbbd; continue;
            }
            0x30dbbd => {
                fa = ebx;
                fb = ((((0x33i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30dbcc; continue; }
                pc = 0x30dbc2; continue;
            }
            0x30dbc2 => {
                fa = ebx;
                fb = ((((0x34i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30dbcc; continue; }
                pc = 0x30dbc7; continue;
            }
            0x30dbc7 => {
                fa = ebx;
                fb = ((((0x37i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x30dbf9; continue; }
                pc = 0x30dbcc; continue;
            }
            0x30dbcc => {
                edx = m.read32(0x7844a8);
                eax = m.read32(esp.wrapping_add(0x38));
                fa = m.read32(edx.wrapping_add(0x2f1c));
                fb = eax;
                if (fa != fb) { pc = 0x30dbf9; continue; }
                pc = 0x30dbde; continue;
            }
            0x30dbde => {
                ecx = m.read32(esp.wrapping_add(0x28));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30dbf9; continue; }
                pc = 0x30dbe6; continue;
            }
            0x30dbe6 => {
                ecx = m.read32(esp.wrapping_add(0x10));
                edx = m.read32(esp.wrapping_add(0x14));
                push!(edi);
                push!(ecx);
                push!(edx);
                call!(0x30d4d0);
                pc = 0x30dbf6; continue;
            }
            0x30dbf6 => {
                esp = esp.wrapping_add(0xc);
                pc = 0x30dbf9; continue;
            }
            0x30dbf9 => {
                edi = m.read32(esp.wrapping_add(0x3c));
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30dd0f; continue; }
                pc = 0x30dc05; continue;
            }
            0x30dc05 => {
                push!(0x12);
                call!(0x3fa10);
                pc = 0x30dc0c; continue;
            }
            0x30dc0c => {
                xmm0.lanes[0] = (((m.read32(0x4e8cf0)) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x47d7c4))).to_bits();
                xmm1 = Xmm::scalar(m.read32(0x783f2c));
                esp = esp.wrapping_add(0x4);
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                ebx = eax;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30dd06; continue; }
                pc = 0x30dc32; continue;
            }
            0x30dc32 => {
                push!(0x3);
                call!(0x80f20);
                pc = 0x30dc39; continue;
            }
            0x30dc39 => {
                xmm2 = Xmm::scalar(m.read32(0x475cd4));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x18)));
                xmm1 = Xmm::scalar(m.read32(0x783f2c));
                xmm3.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xmm4.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = xmm0;
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                esi = eax;
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm2 = xmm1;
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                push!(esi);
                m.write32(esp.wrapping_add(0x2c), xmm3.lanes[0]);
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x34), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0x38), xmm1.lanes[0]);
                call!(0x1ed50);
                pc = 0x30dcc7; continue;
            }
            0x30dcc7 => {
                esi = ((esi as i32).wrapping_mul((0x34) as i32)) as u32;
                eax = m.read32(0x4b85c8);
                ecx = m.read32(eax);
                edx = ((m.read16(esi.wrapping_add(ecx).wrapping_add(0x2a))) as u16 as u32);
                eax = esi.wrapping_add(ecx);
                eax = ((m.read16(eax.wrapping_add(0x26))) as u16 as u32);
                ecx = m.read32(esp.wrapping_add(0x38));
                push!(edi);
                edx = edx.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                push!(edx);
                edx = m.read32(esp.wrapping_add(0x3c));
                eax = eax.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x3c));
                push!(0);
                push!(0);
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x44));
                push!(edx);
                push!(eax);
                push!(ecx);
                call!(0x1b7b0);
                pc = 0x30dd03; continue;
            }
            0x30dd03 => {
                esp = esp.wrapping_add(0x2c);
                pc = 0x30dd06; continue;
            }
            0x30dd06 => {
                push!(ebx);
                call!(0x3fa10);
                pc = 0x30dd0c; continue;
            }
            0x30dd0c => {
                esp = esp.wrapping_add(0x4);
                pc = 0x30dd0f; continue;
            }
            0x30dd0f => {
                fa = ((((m.read16(ebp.wrapping_add(0x2)))) as u32) & 0xffffu32);
                fb = ((((0x1i32)) as u32) & 0xffffu32);
                if (fa != fb) { pc = 0x30dd2d; continue; }
                pc = 0x30dd16; continue;
            }
            0x30dd16 => {
                edx = m.read32(esp.wrapping_add(0x10));
                eax = m.read32(esp.wrapping_add(0x14));
                ecx = m.read32(esp.wrapping_add(0x38));
                push!(edx);
                push!(eax);
                push!(ecx);
                call!(0x3091c0);
                pc = 0x30dd2a; continue;
            }
            0x30dd2a => {
                esp = esp.wrapping_add(0xc);
                pc = 0x30dd2d; continue;
            }
            0x30dd2d => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x24);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0030DD40: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f30dd40<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x30dd40;
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
        { crate::pose::cover(0x30dd40, previous, pc); previous = pc; }
        match pc {
            0x30dd40 => {
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0x8));
                eax = m.read32(ebx.wrapping_add(0xc));
                push!(ebp);
                ebp = m.read32(0x7844a8);
                fpu.push((m.read32(ebp.wrapping_add(0x264)) as i32 as f64));
                push!(esi);
                esi = m.read32(ebx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x2c));
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(ebx.wrapping_add(0x8))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                call!(0x3c848c);
                pc = 0x30dd62; continue;
            }
            0x30dd62 => {
                fa = eax;
                fb = m.read32(esi.wrapping_add(0x1c));
                if (fa >= fb) { pc = 0x30de30; continue; }
                pc = 0x30dd6b; continue;
            }
            0x30dd6b => {
                esi = m.read32(ebx.wrapping_add(0x3c));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30de20; continue; }
                pc = 0x30dd76; continue;
            }
            0x30dd76 => {
                push!(edi);
                pc = 0x30dd80; continue;
            }
            0x30dd79 => {
                ebx = m.read32(esp.wrapping_add(0x14));
                pc = 0x30dd80; continue;
            }
            0x30dd80 => {
                ecx = m.read32(esp.wrapping_add(0x14));
                eax = (((((f32::from_bits(m.read32(ecx.wrapping_add(0x8))) as f64)) as i32)) as u32);
                eax = (eax & 0xffff_0000) | ((((((((eax) as u16)) as i32).wrapping_add(((m.read16(esi.wrapping_add(0x10))) as i32))) as u32)) & 0xffff);
                edi = (((((f32::from_bits(m.read32(ebx)) as f64)) as i32)) as u32);
                ebx = (((((f32::from_bits(m.read32(ebx.wrapping_add(0x4))) as f64)) as i32)) as u32);
                edi = (edi & 0xffff_0000) | ((((((((edi) as u16)) as i32).wrapping_add(((m.read16(esi.wrapping_add(0xc))) as i32))) as u32)) & 0xffff);
                ebx = (ebx & 0xffff_0000) | ((((((((ebx) as u16)) as i32).wrapping_add(((m.read16(esi.wrapping_add(0xe))) as i32))) as u32)) & 0xffff);
                edx = ((((eax) as u16)) as u16 as i16 as i32 as u32);
                fa = edx;
                fb = m.read32(ebp.wrapping_add(0x264));
                if (fa != fb) { pc = 0x30de14; continue; }
                pc = 0x30dda9; continue;
            }
            0x30dda9 => {
                eax = m.read32(esi);
                edx = m.read32(esi.wrapping_add(0x4));
                eax = ((eax as i32).wrapping_mul((0xc4) as i32)) as u32;
                eax = m.read32(eax.wrapping_add(0x524f5c));
                ecx = 0;
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                edx = m.read32(ebp.wrapping_add(0x254));
                ecx = (ecx & 0xffff_ff00) | ((((if ((fas < fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                ecx = ecx.wrapping_sub(1);
                ecx = (ecx & 0x5064787f);
                fa = edx;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x30dddc; continue; }
                pc = 0x30ddd3; continue;
            }
            0x30ddd3 => {
                fa = m.read32(ebp.wrapping_add(0x1185c));
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x30dde1; continue; }
                pc = 0x30dddc; continue;
            }
            0x30dddc => {
                fa = edx;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x30ddf6; continue; }
                pc = 0x30dde1; continue;
            }
            0x30dde1 => {
                eax = (eax & 0xffffff28u32);
                eax = (eax | 0x28);
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30ddf6; continue; }
                pc = 0x30dded; continue;
            }
            0x30dded => {
                ecx = (ecx & 0xffffff28u32);
                ecx = (ecx | 0x28);
                pc = 0x30ddf6; continue;
            }
            0x30ddf6 => {
                push!(ecx);
                ecx = ((((ebx) as u16)) as u16 as i16 as i32 as u32);
                edx = ((((edi) as u16)) as u16 as i16 as i32 as u32);
                push!(esi);
                xh[0].lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                call!(0x30d7b0);
                pc = 0x30de0b; continue;
            }
            0x30de0b => {
                ebp = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x8);
                pc = 0x30de14; continue;
            }
            0x30de14 => {
                esi = m.read32(esi.wrapping_add(0x14));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x30dd79; continue; }
                pc = 0x30de1f; continue;
            }
            0x30de1f => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x30de20; continue;
            }
            0x30de20 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(esp.wrapping_add(0x4), 0x1);
                tail_to!(0x309a80); break 'run;  // direct tail jump
            }
            0x30de30 => {
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

/// 0x0030DE40: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f30de40<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x30de40;
    let mut jt = 0u32;
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
        { crate::pose::cover(0x30de40, previous, pc); previous = pc; }
        match pc {
            0x30de40 => {
                eax = m.read32(esp.wrapping_add(0x10));
                esp = esp.wrapping_sub(0xf8);
                push!(ebx);
                push!(ebp);
                eax = eax.wrapping_sub(1);
                fa = eax;
                fb = ((((0x8i32)) as u32) & 0xffffffffu32);
                push!(esi);
                push!(edi);
                if (fa > fb) { pc = 0x30e070; continue; }
                pc = 0x30de58; continue;
            }
            0x30de58 => {
                jt = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x3100b8));
                if (jt == 0x30de5fu32) { pc = 0x30de5f; continue; }
                if (jt == 0x30dea7u32) { pc = 0x30dea7; continue; }
                if (jt == 0x30deefu32) { pc = 0x30deef; continue; }
                if (jt == 0x30df37u32) { pc = 0x30df37; continue; }
                if (jt == 0x30df7fu32) { pc = 0x30df7f; continue; }
                if (jt == 0x30dfc7u32) { pc = 0x30dfc7; continue; }
                if (jt == 0x30dff2u32) { pc = 0x30dff2; continue; }
                if (jt == 0x30e01du32) { pc = 0x30e01d; continue; }
                if (jt == 0x30e048u32) { pc = 0x30e048; continue; }
                tail_to!(jt); break 'run;  // indirect tail jump, target outside the switch
            }
            0x30de5f => {
                eax = ((m.read8(0x7ba034)) as u8 as u32);
                edi = m.read32(esp.wrapping_add(0x120));
                eax = (eax | 0x54545400);
                push!(edi);
                push!(eax);
                call!(0x154fe0);
                pc = 0x30de79; continue;
            }
            0x30de79 => {
                ecx = ((m.read8(0x7ba034)) as u8 as u32);
                ecx = (ecx | 0x32323200);
                ebp = eax;
                push!(edi);
                push!(ecx);
                m.write32(esp.wrapping_add(0x40), ebp);
                call!(0x154fe0);
                pc = 0x30de93; continue;
            }
            0x30de93 => {
                edx = ((m.read8(0x7ba034)) as u8 as u32);
                push!(edi);
                edx = (edx | 0x1e1e1e00);
                push!(edx);
                pc = 0x30e099; continue;
            }
            0x30dea7 => {
                eax = ((m.read8(0x7ba17c)) as u8 as u32);
                edi = m.read32(esp.wrapping_add(0x120));
                eax = (eax | 0x503c5a00);
                push!(edi);
                push!(eax);
                call!(0x154fe0);
                pc = 0x30dec1; continue;
            }
            0x30dec1 => {
                ecx = ((m.read8(0x7ba17c)) as u8 as u32);
                ecx = (ecx | 0x3c324600);
                ebp = eax;
                push!(edi);
                push!(ecx);
                m.write32(esp.wrapping_add(0x40), ebp);
                call!(0x154fe0);
                pc = 0x30dedb; continue;
            }
            0x30dedb => {
                edx = ((m.read8(0x7ba17c)) as u8 as u32);
                push!(edi);
                edx = (edx | 0xf0f0f00);
                push!(edx);
                pc = 0x30e099; continue;
            }
            0x30deef => {
                eax = ((m.read8(0x7ba17c)) as u8 as u32);
                edi = m.read32(esp.wrapping_add(0x120));
                eax = (eax | 0x7f323200);
                push!(edi);
                push!(eax);
                call!(0x154fe0);
                pc = 0x30df09; continue;
            }
            0x30df09 => {
                ecx = ((m.read8(0x7ba17c)) as u8 as u32);
                ecx = (ecx | 0x3c141400);
                ebp = eax;
                push!(edi);
                push!(ecx);
                m.write32(esp.wrapping_add(0x40), ebp);
                call!(0x154fe0);
                pc = 0x30df23; continue;
            }
            0x30df23 => {
                edx = ((m.read8(0x7ba17c)) as u8 as u32);
                push!(edi);
                edx = (edx | 0x28000000);
                push!(edx);
                pc = 0x30e099; continue;
            }
            0x30df37 => {
                eax = ((m.read8(0x7ba17c)) as u8 as u32);
                edi = m.read32(esp.wrapping_add(0x120));
                eax = (eax | 0x7f7f00);
                push!(edi);
                push!(eax);
                call!(0x154fe0);
                pc = 0x30df51; continue;
            }
            0x30df51 => {
                ecx = ((m.read8(0x7ba17c)) as u8 as u32);
                ecx = (ecx | 0x3f3f00);
                ebp = eax;
                push!(edi);
                push!(ecx);
                m.write32(esp.wrapping_add(0x40), ebp);
                call!(0x154fe0);
                pc = 0x30df6b; continue;
            }
            0x30df6b => {
                edx = ((m.read8(0x7ba17c)) as u8 as u32);
                push!(edi);
                edx = (edx | 0x2b2b00);
                push!(edx);
                pc = 0x30e099; continue;
            }
            0x30df7f => {
                eax = ((m.read8(0x7ba034)) as u8 as u32);
                edi = m.read32(esp.wrapping_add(0x120));
                eax = (eax | 0x647f7f00);
                push!(edi);
                push!(eax);
                call!(0x154fe0);
                pc = 0x30df99; continue;
            }
            0x30df99 => {
                ecx = ((m.read8(0x7ba034)) as u8 as u32);
                ecx = (ecx | 0x3f3f00);
                ebp = eax;
                push!(edi);
                push!(ecx);
                m.write32(esp.wrapping_add(0x40), ebp);
                call!(0x154fe0);
                pc = 0x30dfb3; continue;
            }
            0x30dfb3 => {
                edx = ((m.read8(0x7ba034)) as u8 as u32);
                push!(edi);
                edx = (edx | 0x2b2b00);
                push!(edx);
                pc = 0x30e099; continue;
            }
            0x30dfc7 => {
                edi = m.read32(esp.wrapping_add(0x120));
                push!(edi);
                push!(0x3232325a);
                call!(0x154fe0);
                pc = 0x30dfd9; continue;
            }
            0x30dfd9 => {
                ebp = eax;
                push!(edi);
                push!(0x1414145a);
                m.write32(esp.wrapping_add(0x40), ebp);
                call!(0x154fe0);
                pc = 0x30dfea; continue;
            }
            0x30dfea => {
                push!(edi);
                push!(0x3c);
                pc = 0x30e099; continue;
            }
            0x30dff2 => {
                edi = m.read32(esp.wrapping_add(0x120));
                push!(edi);
                push!(0x507f5040);
                call!(0x154fe0);
                pc = 0x30e004; continue;
            }
            0x30e004 => {
                ebp = eax;
                push!(edi);
                push!(0x1e321e40);
                m.write32(esp.wrapping_add(0x40), ebp);
                call!(0x154fe0);
                pc = 0x30e015; continue;
            }
            0x30e015 => {
                push!(edi);
                push!(0xa1e0a40);
                pc = 0x30e099; continue;
            }
            0x30e01d => {
                edi = m.read32(esp.wrapping_add(0x120));
                push!(edi);
                push!(0x40404064);
                call!(0x154fe0);
                pc = 0x30e02f; continue;
            }
            0x30e02f => {
                ebp = eax;
                push!(edi);
                push!(0x1e1e1e2d);
                m.write32(esp.wrapping_add(0x40), ebp);
                call!(0x154fe0);
                pc = 0x30e040; continue;
            }
            0x30e040 => {
                push!(edi);
                push!(0xf0f0f2d);
                pc = 0x30e099; continue;
            }
            0x30e048 => {
                edi = m.read32(esp.wrapping_add(0x120));
                push!(edi);
                push!(0x5a5a5a78);
                call!(0x154fe0);
                pc = 0x30e05a; continue;
            }
            0x30e05a => {
                ebp = eax;
                push!(edi);
                push!(0x3c3c3c5a);
                m.write32(esp.wrapping_add(0x40), ebp);
                call!(0x154fe0);
                pc = 0x30e06b; continue;
            }
            0x30e06b => {
                push!(edi);
                push!(0x1);
                pc = 0x30e099; continue;
            }
            0x30e070 => {
                edi = m.read32(esp.wrapping_add(0x120));
                push!(edi);
                push!(0x5a5a5a80);
                call!(0x154fe0);
                pc = 0x30e082; continue;
            }
            0x30e082 => {
                ebp = eax;
                push!(edi);
                push!(0x2828285a);
                m.write32(esp.wrapping_add(0x40), ebp);
                call!(0x154fe0);
                pc = 0x30e093; continue;
            }
            0x30e093 => {
                push!(edi);
                push!(0x1414145a);
                pc = 0x30e099; continue;
            }
            0x30e099 => {
                esi = eax;
                call!(0x154fe0);
                pc = 0x30e0a0; continue;
            }
            0x30e0a0 => {
                edx = m.read32(0x7844a8);
                edi = m.read32(esp.wrapping_add(0x12c));
                m.write32(esp.wrapping_add(0x34), eax);
                eax = m.read32(esp.wrapping_add(0x134));
                ecx = (((((f32::from_bits(m.read32(eax.wrapping_add(0x8))) as f64)) as i32)) as u32);
                eax = m.read32(edx.wrapping_add(0x264));
                eax = eax.wrapping_sub(ecx);
                ecx = m.read32(edi.wrapping_add(0x1c));
                esp = esp.wrapping_add(0x18);
                fa = eax;
                fb = ecx;
                m.write32(esp.wrapping_add(0x48), eax);
                if (fa < fb) { pc = 0x30e0d8; continue; }
                pc = 0x30e0d3; continue;
            }
            0x30e0d3 => {
                ecx = ecx.wrapping_sub(1);
                m.write32(esp.wrapping_add(0x48), ecx);
                pc = 0x30e0d8; continue;
            }
            0x30e0d8 => {
                eax = m.read32(edi.wrapping_add(0x18));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x18), 0);
                if (fas <= fbs) { pc = 0x30e753; continue; }
                pc = 0x30e0eb; continue;
            }
            0x30e0eb => {
                pc = 0x30e0f0; continue;
            }
            0x30e0f0 => {
                eax = m.read32(esp.wrapping_add(0x114));
                eax = m.read32(eax.wrapping_add(0x14));
                xmm0.lanes[0] = (((m.read32(esp.wrapping_add(0x18))) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x110)))).to_bits();
                edi = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                if (fas <= fbs) { pc = 0x30e736; continue; }
                pc = 0x30e119; continue;
            }
            0x30e119 => {
                pc = 0x30e120; continue;
            }
            0x30e120 => {
                eax = ((eax as i32).wrapping_mul((m.read32(esp.wrapping_add(0x18))) as i32)) as u32;
                ecx = m.read32(esp.wrapping_add(0x114));
                edx = m.read32(esp.wrapping_add(0x48));
                ecx = m.read32(ecx.wrapping_add(edx.wrapping_mul(0x4)).wrapping_add(0x20));
                eax = eax.wrapping_add(edi);
                ebx = (ebx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(eax.wrapping_mul(0x4)))) as u32)) & 0xffff);
                fa = ((((((ebx) as u16))) as u32) & 0xffffu32);
                fb = ((((0i32)) as u32) & 0xffffu32);
                if (fa == fb) { pc = 0x30e723; continue; }
                pc = 0x30e143; continue;
            }
            0x30e143 => {
                eax = m.read32(0x78400c);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                xmm1.lanes[0] = (((edi) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x10c)))).to_bits();
                m.write32(esp.wrapping_add(0x14), xmm1.lanes[0]);
                if (fa == fb) { pc = 0x30e26b; continue; }
                pc = 0x30e163; continue;
            }
            0x30e163 => {
                eax = m.read32(esp.wrapping_add(0x11c));
                push!(edx);
                push!(eax);
                call!(0x334730);
                pc = 0x30e171; continue;
            }
            0x30e171 => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x1c)));
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x30e265; continue; }
                pc = 0x30e182; continue;
            }
            0x30e182 => {
                xmm4 = Xmm::scalar(m.read32(0x475c78));
                xmm0 = Xmm::scalar(m.read32(0x783dd8));
                xh[1].lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x10)));
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xh[2] = Xmm::scalar(m.read32(0x783f2c));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xh[0].lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xmm3 = xmm1;
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x10)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x40), xmm0.lanes[0]);
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x34), xmm3.lanes[0]);
                m.write32(esp.wrapping_add(0x20), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0x4c), xmm1.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30e22c; continue; }
                pc = 0x30e222; continue;
            }
            0x30e222 => {
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x40), xmm2.lanes[0]);
                pc = 0x30e22c; continue;
            }
            0x30e22c => {
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30e23f; continue; }
                pc = 0x30e235; continue;
            }
            0x30e235 => {
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x4c), xmm3.lanes[0]);
                pc = 0x30e23f; continue;
            }
            0x30e23f => {
                ecx = m.read32(esp.wrapping_add(0x1c));
                edx = m.read32(esp.wrapping_add(0x40));
                eax = m.read32(esp.wrapping_add(0x4c));
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x24));
                push!(edx);
                edx = m.read32(esp.wrapping_add(0x3c));
                push!(eax);
                push!(ecx);
                push!(edx);
                call!(0x1a2e0);
                pc = 0x30e25d; continue;
            }
            0x30e25d => {
                esp = esp.wrapping_add(0x14);
                pc = 0x30e71d; continue;
            }
            0x30e265 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x10)));
                pc = 0x30e26b; continue;
            }
            0x30e26b => {
                fa = (((((((ebx) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x5i32)) as u32) & 0xffu32);
                xmm2 = xmm0;
                if ((fa & fb) != 0) { pc = 0x30e50a; continue; }
                pc = 0x30e277; continue;
            }
            0x30e277 => {
                fa = (((((((ebx) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0xai32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x30e2e8; continue; }
                pc = 0x30e27c; continue;
            }
            0x30e27c => {
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                if (((((((((((((fa)) as u8)) as i32).wrapping_sub((((((fb)) as u8)) as i32)))) as u8)) as i32).wrapping_shr((0x7 & 31))) == 0i32)) { pc = 0x30e293; continue; }
                pc = 0x30e290; continue;
            }
            0x30e290 => {
                push!(ebp);
                pc = 0x30e29d; continue;
            }
            0x30e293 => {
                fa = (((((((ebx) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x30e2c2; continue; }
                pc = 0x30e298; continue;
            }
            0x30e298 => {
                edx = m.read32(esp.wrapping_add(0x1c));
                push!(edx);
                pc = 0x30e29d; continue;
            }
            0x30e29d => {
                eax = m.read32(esp.wrapping_add(0x14));
                ecx = m.read32(esp.wrapping_add(0x18));
                esp = esp.wrapping_sub(0x8);
                m.write32(esp.wrapping_add(0x4), xmm2.lanes[0]);
                m.write32(esp, xmm1.lanes[0]);
                push!(eax);
                push!(ecx);
                call!(0x308b10);
                pc = 0x30e2ba; continue;
            }
            0x30e2ba => {
                esp = esp.wrapping_add(0x14);
                pc = 0x30e71d; continue;
            }
            0x30e2c2 => {
                edx = m.read32(esp.wrapping_add(0x10));
                eax = m.read32(esp.wrapping_add(0x14));
                push!(esi);
                esp = esp.wrapping_sub(0x8);
                m.write32(esp.wrapping_add(0x4), xmm2.lanes[0]);
                m.write32(esp, xmm1.lanes[0]);
                push!(edx);
                push!(eax);
                call!(0x308b10);
                pc = 0x30e2e0; continue;
            }
            0x30e2e0 => {
                esp = esp.wrapping_add(0x14);
                pc = 0x30e71d; continue;
            }
            0x30e2e8 => {
                xmm4 = Xmm::scalar(m.read32(0x475c78));
                xh[1].lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xh[2] = Xmm::scalar(m.read32(0x783dd8));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xh[0].lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xmm3 = xmm1;
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x2c), xmm1.lanes[0]);
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(0x783f2c))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783f2c))).to_bits();
                m.write32(esp.wrapping_add(0x54), xmm0.lanes[0]);
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x90), xmm3.lanes[0]);
                m.write32(esp.wrapping_add(0x7c), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0x5c), xmm1.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30e38f; continue; }
                pc = 0x30e385; continue;
            }
            0x30e385 => {
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x54), xmm2.lanes[0]);
                pc = 0x30e38f; continue;
            }
            0x30e38f => {
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30e3a2; continue; }
                pc = 0x30e398; continue;
            }
            0x30e398 => {
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x5c), xmm3.lanes[0]);
                pc = 0x30e3a2; continue;
            }
            0x30e3a2 => {
                ecx = m.read32(esp.wrapping_add(0x1c));
                edx = m.read32(esp.wrapping_add(0x54));
                eax = m.read32(esp.wrapping_add(0x5c));
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x80));
                push!(edx);
                edx = m.read32(esp.wrapping_add(0x98));
                push!(eax);
                push!(ecx);
                push!(edx);
                call!(0x1a2e0);
                pc = 0x30e3c6; continue;
            }
            0x30e3c6 => {
                xmm0 = Xmm::default();
                m.write32(esp.wrapping_add(0x38), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475ce8))).to_bits();
                esp = esp.wrapping_add(0x14);
                m.write32(esp.wrapping_add(0x88), xmm0.lanes[0]);
                pc = 0x30e3f0; continue;
            }
            0x30e3f0 => {
                xh[1] = Xmm::scalar(m.read32(0x783dd8));
                xmm0 = Xmm::scalar(m.read32(0x783f2c));
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xmm3 = Xmm::scalar(m.read32(esp.wrapping_add(0x2c)));
                xh[2] = Xmm::scalar(m.read32(0x783cc0));
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x10)))).to_bits();
                xmm4.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xh[0].lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x88)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x24)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = Xmm::scalar(m.read32(0x475c78));
                m.write32(esp.wrapping_add(0x64), xmm0.lanes[0]);
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x74), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0x80), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x6c), xmm3.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30e4ad; continue; }
                pc = 0x30e4a3; continue;
            }
            0x30e4a3 => {
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x64), xmm1.lanes[0]);
                pc = 0x30e4ad; continue;
            }
            0x30e4ad => {
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm3.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30e4c0; continue; }
                pc = 0x30e4b6; continue;
            }
            0x30e4b6 => {
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x6c), xmm2.lanes[0]);
                pc = 0x30e4c0; continue;
            }
            0x30e4c0 => {
                eax = m.read32(esp.wrapping_add(0x64));
                ecx = m.read32(esp.wrapping_add(0x6c));
                edx = m.read32(esp.wrapping_add(0x80));
                push!(esi);
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x7c));
                push!(ecx);
                push!(edx);
                push!(eax);
                call!(0x1a2e0);
                pc = 0x30e4dd; continue;
            }
            0x30e4dd => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x38)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d24))).to_bits();
                xmm1 = Xmm::scalar(m.read32(0x475c78));
                esp = esp.wrapping_add(0x14);
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                if ((fca > fcb)) { pc = 0x30e3f0; continue; }
                pc = 0x30e505; continue;
            }
            0x30e505 => {
                pc = 0x30e71d; continue;
            }
            0x30e50a => {
                xmm4 = Xmm::scalar(m.read32(0x475c78));
                xh[1].lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xh[2] = Xmm::scalar(m.read32(0x783dd8));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xh[0].lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x28), xmm0.lanes[0]);
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm3 = xmm1;
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(0x783f2c))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783f2c))).to_bits();
                m.write32(esp.wrapping_add(0x60), xmm0.lanes[0]);
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x70), xmm3.lanes[0]);
                m.write32(esp.wrapping_add(0x44), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0x58), xmm1.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30e5ae; continue; }
                pc = 0x30e5a4; continue;
            }
            0x30e5a4 => {
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x60), xmm2.lanes[0]);
                pc = 0x30e5ae; continue;
            }
            0x30e5ae => {
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30e5c1; continue; }
                pc = 0x30e5b7; continue;
            }
            0x30e5b7 => {
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x58), xmm3.lanes[0]);
                pc = 0x30e5c1; continue;
            }
            0x30e5c1 => {
                ecx = m.read32(esp.wrapping_add(0x1c));
                edx = m.read32(esp.wrapping_add(0x60));
                eax = m.read32(esp.wrapping_add(0x58));
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x48));
                push!(edx);
                edx = m.read32(esp.wrapping_add(0x78));
                push!(eax);
                push!(ecx);
                push!(edx);
                call!(0x1a2e0);
                pc = 0x30e5df; continue;
            }
            0x30e5df => {
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x28)));
                xh[2] = Xmm::default();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x475ce8))).to_bits();
                esp = esp.wrapping_add(0x14);
                m.write32(esp.wrapping_add(0x3c), xh[2].lanes[0]);
                m.write32(esp.wrapping_add(0x78), xmm2.lanes[0]);
                pc = 0x30e610; continue;
            }
            0x30e601 => {
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x78)));
                xh[2] = Xmm::scalar(m.read32(esp.wrapping_add(0x3c)));
                pc = 0x30e610; continue;
            }
            0x30e610 => {
                xh[0] = Xmm::scalar(m.read32(0x783dd8));
                xmm0.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xh[1] = Xmm::scalar(m.read32(0x783f2c));
                xmm4.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                xmm3 = Xmm::scalar(m.read32(esp.wrapping_add(0x10)));
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm1 = xh[2];
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x14)))).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x28)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = Xmm::scalar(m.read32(0x475c78));
                m.write32(esp.wrapping_add(0x68), xmm0.lanes[0]);
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x8c), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x84), xmm3.lanes[0]);
                m.write32(esp.wrapping_add(0x50), xmm2.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30e6c2; continue; }
                pc = 0x30e6b8; continue;
            }
            0x30e6b8 => {
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x68), xmm3.lanes[0]);
                pc = 0x30e6c2; continue;
            }
            0x30e6c2 => {
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm2.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30e6d5; continue; }
                pc = 0x30e6cb; continue;
            }
            0x30e6cb => {
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x50), xmm1.lanes[0]);
                pc = 0x30e6d5; continue;
            }
            0x30e6d5 => {
                eax = m.read32(esp.wrapping_add(0x68));
                ecx = m.read32(esp.wrapping_add(0x50));
                edx = m.read32(esp.wrapping_add(0x84));
                push!(esi);
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x94));
                push!(ecx);
                push!(edx);
                push!(eax);
                call!(0x1a2e0);
                pc = 0x30e6f5; continue;
            }
            0x30e6f5 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x50)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d24))).to_bits();
                xmm1 = Xmm::scalar(m.read32(0x475c78));
                esp = esp.wrapping_add(0x14);
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x3c), xmm0.lanes[0]);
                if ((fca > fcb)) { pc = 0x30e601; continue; }
                pc = 0x30e71d; continue;
            }
            0x30e71d => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x10)));
                pc = 0x30e723; continue;
            }
            0x30e723 => {
                ecx = m.read32(esp.wrapping_add(0x114));
                eax = m.read32(ecx.wrapping_add(0x14));
                edi = edi.wrapping_add(1);
                fa = edi;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x30e120; continue; }
                pc = 0x30e736; continue;
            }
            0x30e736 => {
                eax = m.read32(esp.wrapping_add(0x18));
                edx = m.read32(esp.wrapping_add(0x114));
                ecx = m.read32(edx.wrapping_add(0x18));
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x18), eax);
                if (fas < fbs) { pc = 0x30e0f0; continue; }
                pc = 0x30e751; continue;
            }
            0x30e751 => {
                edi = edx;
                pc = 0x30e753; continue;
            }
            0x30e753 => {
                eax = m.read32(edi.wrapping_add(0x18));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x18), 0);
                if (fas <= fbs) { pc = 0x30f664; continue; }
                pc = 0x30e766; continue;
            }
            0x30e766 => {
                eax = m.read32(edi.wrapping_add(0x14));
                xmm1.lanes[0] = (((m.read32(esp.wrapping_add(0x18))) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x110)))).to_bits();
                esi = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x10), xmm1.lanes[0]);
                if (fas <= fbs) { pc = 0x30f650; continue; }
                pc = 0x30e788; continue;
            }
            0x30e788 => {
                pc = 0x30e790; continue;
            }
            0x30e790 => {
                ecx = m.read32(esp.wrapping_add(0x48));
                ecx = m.read32(edi.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x20));
                edx = eax;
                edx = ((edx as i32).wrapping_mul((m.read32(esp.wrapping_add(0x18))) as i32)) as u32;
                edx = edx.wrapping_add(esi);
                edx = m.read32(ecx.wrapping_add(edx.wrapping_mul(0x4)));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0x1c), edx);
                if (fa == fb) { pc = 0x30f644; continue; }
                pc = 0x30e7b0; continue;
            }
            0x30e7b0 => {
                ebx = (ebx & 0xffff_ff00) | ((0) & 0xff);
                fa = ((((((edx) as u8))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                xmm0.lanes[0] = (((esi) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x10c)))).to_bits();
                m.write32(esp.wrapping_add(0x14), xmm0.lanes[0]);
                if ((fa & fb) != 0) { pc = 0x30e8e3; continue; }
                pc = 0x30e7ce; continue;
            }
            0x30e7ce => {
                ebp = m.read32(esp.wrapping_add(0x18));
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30e7e6; continue; }
                pc = 0x30e7d6; continue;
            }
            0x30e7d6 => {
                ebp = ebp.wrapping_sub(1);
                ebp = ((ebp as i32).wrapping_mul((eax) as i32)) as u32;
                ebp = ebp.wrapping_add(esi);
                fa = m.read32(ecx.wrapping_add(ebp.wrapping_mul(0x4)));
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x30e8df; continue; }
                pc = 0x30e7e6; continue;
            }
            0x30e7e6 => {
                xh[0].lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                xh[1] = Xmm::scalar(m.read32(0x475c78));
                xh[2] = Xmm::scalar(m.read32(0x783dd8));
                xmm4.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xmm2 = xmm1;
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x475d50))).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm3 = xmm0;
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(0x783f2c))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783f2c))).to_bits();
                m.write32(esp.wrapping_add(0x50), xmm1.lanes[0]);
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                fca = f32::from_bits(xh[1].lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x104), xmm3.lanes[0]);
                m.write32(esp.wrapping_add(0xc0), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0x68), xmm0.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30e891; continue; }
                pc = 0x30e887; continue;
            }
            0x30e887 => {
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x50), xmm2.lanes[0]);
                pc = 0x30e891; continue;
            }
            0x30e891 => {
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                fca = f32::from_bits(xh[1].lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30e8a4; continue; }
                pc = 0x30e89a; continue;
            }
            0x30e89a => {
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x68), xmm3.lanes[0]);
                pc = 0x30e8a4; continue;
            }
            0x30e8a4 => {
                ebp = m.read32(esp.wrapping_add(0x30));
                eax = m.read32(esp.wrapping_add(0x50));
                ecx = m.read32(esp.wrapping_add(0x68));
                edx = m.read32(esp.wrapping_add(0xc0));
                push!(ebp);
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x10c));
                push!(ecx);
                push!(edx);
                push!(eax);
                call!(0x1a2e0);
                pc = 0x30e8c8; continue;
            }
            0x30e8c8 => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x28)));
                edx = m.read32(esp.wrapping_add(0x30));
                esp = esp.wrapping_add(0x14);
                ebx = (ebx & 0xffff_ff00) | ((0x3) & 0xff);
                pc = 0x30e8e3; continue;
            }
            0x30e8df => {
                ebp = m.read32(esp.wrapping_add(0x30));
                pc = 0x30e8e3; continue;
            }
            0x30e8e3 => {
                fa = ((((((edx) as u8))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x30ea19; continue; }
                pc = 0x30e8ec; continue;
            }
            0x30e8ec => {
                eax = m.read32(edi.wrapping_add(0x14));
                ecx = eax.wrapping_add(((0x1i32.wrapping_neg()) as u32));
                fa = esi;
                fb = ecx;
                if (fa == fb) { pc = 0x30e910; continue; }
                pc = 0x30e8f6; continue;
            }
            0x30e8f6 => {
                eax = ((eax as i32).wrapping_mul((m.read32(esp.wrapping_add(0x18))) as i32)) as u32;
                ecx = m.read32(esp.wrapping_add(0x48));
                ecx = m.read32(edi.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x20));
                eax = eax.wrapping_add(esi);
                fa = m.read32(ecx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x4));
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x30ea19; continue; }
                pc = 0x30e910; continue;
            }
            0x30e910 => {
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                xmm3 = Xmm::scalar(m.read32(0x475d50));
                if ((fa & fb) != 0) { pc = 0x30e920; continue; }
                pc = 0x30e91d; continue;
            }
            0x30e91d => {
                xmm3 = Xmm::default();
                pc = 0x30e920; continue;
            }
            0x30e920 => {
                xmm4 = Xmm::scalar(m.read32(0x475c78));
                xh[2] = Xmm::scalar(m.read32(0x783f28));
                xh[1].lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                xh[0].lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm2 = xmm0;
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x475d50))).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783f2c))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783f2c))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x58), xmm1.lanes[0]);
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0xe0), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0xb8), xmm3.lanes[0]);
                m.write32(esp.wrapping_add(0x60), xmm0.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30e9d0; continue; }
                pc = 0x30e9c6; continue;
            }
            0x30e9c6 => {
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x58), xmm3.lanes[0]);
                pc = 0x30e9d0; continue;
            }
            0x30e9d0 => {
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30e9e3; continue; }
                pc = 0x30e9d9; continue;
            }
            0x30e9d9 => {
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x60), xmm2.lanes[0]);
                pc = 0x30e9e3; continue;
            }
            0x30e9e3 => {
                edx = m.read32(esp.wrapping_add(0x58));
                eax = m.read32(esp.wrapping_add(0x60));
                ecx = m.read32(esp.wrapping_add(0xb8));
                push!(ebp);
                push!(edx);
                edx = m.read32(esp.wrapping_add(0xe8));
                push!(eax);
                push!(ecx);
                push!(edx);
                call!(0x1a2e0);
                pc = 0x30ea03; continue;
            }
            0x30ea03 => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x28)));
                edx = m.read32(esp.wrapping_add(0x30));
                esp = esp.wrapping_add(0x14);
                ebx = (ebx & 0xffff_ff00) | (((((((((ebx) as u8)) as i32) | 0xai32)) as u32)) & 0xff);
                pc = 0x30ea19; continue;
            }
            0x30ea19 => {
                fa = ((((((edx) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x30eb5b; continue; }
                pc = 0x30ea22; continue;
            }
            0x30ea22 => {
                ecx = m.read32(edi.wrapping_add(0x18));
                eax = m.read32(esp.wrapping_add(0x18));
                ecx = ecx.wrapping_sub(1);
                fa = eax;
                fb = ecx;
                if (fa == fb) { pc = 0x30ea47; continue; }
                pc = 0x30ea2e; continue;
            }
            0x30ea2e => {
                ecx = m.read32(esp.wrapping_add(0x48));
                ecx = m.read32(edi.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x20));
                eax = eax.wrapping_add(1);
                eax = ((eax as i32).wrapping_mul((m.read32(edi.wrapping_add(0x14))) as i32)) as u32;
                eax = eax.wrapping_add(esi);
                fa = m.read32(ecx.wrapping_add(eax.wrapping_mul(0x4)));
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x30eb5b; continue; }
                pc = 0x30ea47; continue;
            }
            0x30ea47 => {
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((0x8i32)) as u32) & 0xffu32);
                xmm2 = Xmm::scalar(m.read32(0x475d50));
                if ((fa & fb) != 0) { pc = 0x30ea57; continue; }
                pc = 0x30ea54; continue;
            }
            0x30ea54 => {
                xmm2 = Xmm::default();
                pc = 0x30ea57; continue;
            }
            0x30ea57 => {
                xmm4 = Xmm::scalar(m.read32(0x475c78));
                xh[1].lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                xh[0].lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                m.write32(esp.wrapping_add(0x6c), xmm2.lanes[0]);
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm3 = xmm0;
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(0x783f2c))).to_bits();
                xmm2 = xmm1;
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x475d50))).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x6c)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783f2c))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x64), xmm1.lanes[0]);
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x100), xmm3.lanes[0]);
                m.write32(esp.wrapping_add(0xb0), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0x5c), xmm0.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30eb12; continue; }
                pc = 0x30eb08; continue;
            }
            0x30eb08 => {
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x64), xmm2.lanes[0]);
                pc = 0x30eb12; continue;
            }
            0x30eb12 => {
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30eb25; continue; }
                pc = 0x30eb1b; continue;
            }
            0x30eb1b => {
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x5c), xmm3.lanes[0]);
                pc = 0x30eb25; continue;
            }
            0x30eb25 => {
                edx = m.read32(esp.wrapping_add(0x64));
                eax = m.read32(esp.wrapping_add(0x5c));
                ecx = m.read32(esp.wrapping_add(0xb0));
                push!(ebp);
                push!(edx);
                edx = m.read32(esp.wrapping_add(0x108));
                push!(eax);
                push!(ecx);
                push!(edx);
                call!(0x1a2e0);
                pc = 0x30eb45; continue;
            }
            0x30eb45 => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x28)));
                edx = m.read32(esp.wrapping_add(0x30));
                esp = esp.wrapping_add(0x14);
                ebx = (ebx & 0xffff_ff00) | (((((((((ebx) as u8)) as i32) | 0xci32)) as u32)) & 0xff);
                pc = 0x30eb5b; continue;
            }
            0x30eb5b => {
                fa = ((((((edx) as u8))) as u32) & 0xffu32);
                fb = ((((0x8i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x30ecac; continue; }
                pc = 0x30eb64; continue;
            }
            0x30eb64 => {
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30eb85; continue; }
                pc = 0x30eb68; continue;
            }
            0x30eb68 => {
                eax = m.read32(edi.wrapping_add(0x14));
                eax = ((eax as i32).wrapping_mul((m.read32(esp.wrapping_add(0x18))) as i32)) as u32;
                ecx = m.read32(esp.wrapping_add(0x48));
                ecx = m.read32(edi.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x20));
                eax = eax.wrapping_add(esi);
                fa = ((m.read32(ecx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(((0x4i32.wrapping_neg()) as u32)))) & 0xffffffffu32);
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x30ecac; continue; }
                pc = 0x30eb85; continue;
            }
            0x30eb85 => {
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                xmm2 = Xmm::scalar(m.read32(0x475d50));
                if ((fa & fb) != 0) { pc = 0x30eb95; continue; }
                pc = 0x30eb92; continue;
            }
            0x30eb92 => {
                xmm2 = Xmm::default();
                pc = 0x30eb95; continue;
            }
            0x30eb95 => {
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                m.write32(esp.wrapping_add(0x54), xmm2.lanes[0]);
                xmm2 = Xmm::scalar(m.read32(0x475d50));
                if ((fa & fb) != 0) { pc = 0x30ebab; continue; }
                pc = 0x30eba8; continue;
            }
            0x30eba8 => {
                xmm2 = Xmm::default();
                pc = 0x30ebab; continue;
            }
            0x30ebab => {
                xh[1] = Xmm::scalar(m.read32(0x783dd8));
                xmm4.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xh[0].lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                xmm3 = xmm0;
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d50))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(0x783f2c))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783f2c))).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = Xmm::scalar(m.read32(0x475c78));
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x54)))).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x4c), xmm1.lanes[0]);
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0xe8), xmm3.lanes[0]);
                m.write32(esp.wrapping_add(0xc8), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0x98), xmm0.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30ec60; continue; }
                pc = 0x30ec56; continue;
            }
            0x30ec56 => {
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x4c), xmm2.lanes[0]);
                pc = 0x30ec60; continue;
            }
            0x30ec60 => {
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30ec76; continue; }
                pc = 0x30ec69; continue;
            }
            0x30ec69 => {
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x98), xmm3.lanes[0]);
                pc = 0x30ec76; continue;
            }
            0x30ec76 => {
                edx = m.read32(esp.wrapping_add(0x4c));
                eax = m.read32(esp.wrapping_add(0x98));
                ecx = m.read32(esp.wrapping_add(0xc8));
                push!(ebp);
                push!(edx);
                edx = m.read32(esp.wrapping_add(0xf0));
                push!(eax);
                push!(ecx);
                push!(edx);
                call!(0x1a2e0);
                pc = 0x30ec99; continue;
            }
            0x30ec99 => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x28)));
                edx = m.read32(esp.wrapping_add(0x30));
                esp = esp.wrapping_add(0x14);
                pc = 0x30ecac; continue;
            }
            0x30ecac => {
                fa = edx;
                fb = ((((0x10000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) { pc = 0x30edc3; continue; }
                pc = 0x30ecb8; continue;
            }
            0x30ecb8 => {
                xh[0].lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                xh[1] = Xmm::scalar(m.read32(0x475c78));
                xh[2] = Xmm::scalar(m.read32(0x783dd8));
                xmm4.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xmm2 = xmm1;
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x475dc4))).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x475dc4))).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm3 = xmm0;
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(0x783f2c))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783f2c))).to_bits();
                m.write32(esp.wrapping_add(0xa8), xmm1.lanes[0]);
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                fca = f32::from_bits(xh[1].lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0xf8), xmm3.lanes[0]);
                m.write32(esp.wrapping_add(0xd0), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0x9c), xmm0.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30ed74; continue; }
                pc = 0x30ed67; continue;
            }
            0x30ed67 => {
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0xa8), xmm2.lanes[0]);
                pc = 0x30ed74; continue;
            }
            0x30ed74 => {
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                fca = f32::from_bits(xh[1].lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30ed8a; continue; }
                pc = 0x30ed7d; continue;
            }
            0x30ed7d => {
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x9c), xmm3.lanes[0]);
                pc = 0x30ed8a; continue;
            }
            0x30ed8a => {
                eax = m.read32(esp.wrapping_add(0xa8));
                ecx = m.read32(esp.wrapping_add(0x9c));
                edx = m.read32(esp.wrapping_add(0xd0));
                push!(ebp);
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x100));
                push!(ecx);
                push!(edx);
                push!(eax);
                call!(0x1a2e0);
                pc = 0x30edb0; continue;
            }
            0x30edb0 => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x28)));
                edx = m.read32(esp.wrapping_add(0x30));
                esp = esp.wrapping_add(0x14);
                pc = 0x30edc3; continue;
            }
            0x30edc3 => {
                fa = edx;
                fb = ((((0x20000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) { pc = 0x30eede; continue; }
                pc = 0x30edcf; continue;
            }
            0x30edcf => {
                xmm4 = Xmm::scalar(m.read32(0x475c78));
                xh[1].lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                xh[0].lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xmm3 = xmm1;
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm2 = xmm0;
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x49f7bc))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783f2c))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x475dc4))).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783f2c))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0xa4), xmm1.lanes[0]);
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0xf0), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0xd8), xmm3.lanes[0]);
                m.write32(esp.wrapping_add(0x94), xmm0.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30ee8f; continue; }
                pc = 0x30ee82; continue;
            }
            0x30ee82 => {
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0xa4), xmm3.lanes[0]);
                pc = 0x30ee8f; continue;
            }
            0x30ee8f => {
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30eea5; continue; }
                pc = 0x30ee98; continue;
            }
            0x30ee98 => {
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x94), xmm2.lanes[0]);
                pc = 0x30eea5; continue;
            }
            0x30eea5 => {
                ecx = m.read32(esp.wrapping_add(0xa4));
                edx = m.read32(esp.wrapping_add(0x94));
                eax = m.read32(esp.wrapping_add(0xd8));
                push!(ebp);
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0xf8));
                push!(edx);
                push!(eax);
                push!(ecx);
                call!(0x1a2e0);
                pc = 0x30eecb; continue;
            }
            0x30eecb => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x28)));
                edx = m.read32(esp.wrapping_add(0x30));
                esp = esp.wrapping_add(0x14);
                pc = 0x30eede; continue;
            }
            0x30eede => {
                fa = edx;
                fb = ((((0x40000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) { pc = 0x30eff9; continue; }
                pc = 0x30eeea; continue;
            }
            0x30eeea => {
                xmm4 = Xmm::scalar(m.read32(0x475c78));
                xh[1].lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                xh[0].lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xmm2 = xmm1;
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x49f7bc))).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x475dc4))).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm3 = xmm0;
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(0x783f2c))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783f2c))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0xac), xmm1.lanes[0]);
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0xbc), xmm3.lanes[0]);
                m.write32(esp.wrapping_add(0xb4), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0xa0), xmm0.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30efaa; continue; }
                pc = 0x30ef9d; continue;
            }
            0x30ef9d => {
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0xac), xmm2.lanes[0]);
                pc = 0x30efaa; continue;
            }
            0x30efaa => {
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30efc0; continue; }
                pc = 0x30efb3; continue;
            }
            0x30efb3 => {
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0xa0), xmm3.lanes[0]);
                pc = 0x30efc0; continue;
            }
            0x30efc0 => {
                edx = m.read32(esp.wrapping_add(0xac));
                eax = m.read32(esp.wrapping_add(0xa0));
                ecx = m.read32(esp.wrapping_add(0xb4));
                push!(ebp);
                push!(edx);
                edx = m.read32(esp.wrapping_add(0xc4));
                push!(eax);
                push!(ecx);
                push!(edx);
                call!(0x1a2e0);
                pc = 0x30efe6; continue;
            }
            0x30efe6 => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x28)));
                edx = m.read32(esp.wrapping_add(0x30));
                esp = esp.wrapping_add(0x14);
                pc = 0x30eff9; continue;
            }
            0x30eff9 => {
                fa = edx;
                fb = ((((0x80000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) { pc = 0x30f110; continue; }
                pc = 0x30f005; continue;
            }
            0x30f005 => {
                xh[1] = Xmm::scalar(m.read32(0x783dd8));
                xmm4.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xh[0].lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                xmm2 = xmm0;
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x475dc4))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475dc4))).to_bits();
                xmm3 = xmm1;
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783f2c))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783f2c))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = Xmm::scalar(m.read32(0x475c78));
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x8c), xmm1.lanes[0]);
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0xcc), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0xc4), xmm3.lanes[0]);
                m.write32(esp.wrapping_add(0x84), xmm0.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30f0c1; continue; }
                pc = 0x30f0b4; continue;
            }
            0x30f0b4 => {
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x8c), xmm3.lanes[0]);
                pc = 0x30f0c1; continue;
            }
            0x30f0c1 => {
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30f0d7; continue; }
                pc = 0x30f0ca; continue;
            }
            0x30f0ca => {
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x84), xmm2.lanes[0]);
                pc = 0x30f0d7; continue;
            }
            0x30f0d7 => {
                eax = m.read32(esp.wrapping_add(0x8c));
                ecx = m.read32(esp.wrapping_add(0x84));
                edx = m.read32(esp.wrapping_add(0xc4));
                push!(ebp);
                push!(eax);
                eax = m.read32(esp.wrapping_add(0xd4));
                push!(ecx);
                push!(edx);
                push!(eax);
                call!(0x1a2e0);
                pc = 0x30f0fd; continue;
            }
            0x30f0fd => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x28)));
                edx = m.read32(esp.wrapping_add(0x30));
                esp = esp.wrapping_add(0x14);
                pc = 0x30f110; continue;
            }
            0x30f110 => {
                fa = edx;
                fb = ((((0x100000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) { pc = 0x30f3a8; continue; }
                pc = 0x30f11c; continue;
            }
            0x30f11c => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x10))) as f64));
                push!(ebp);
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475c78)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475dc4))).to_bits();
                push!(ebp);
                push!(ebp);
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(0x475dc4)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(ebp);
                m.write32(esp.wrapping_add(0x54), (fpu.st(0) as f32).to_bits()); fpu.pop();
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x24))) as f64));
                ecx = m.read32(esp.wrapping_add(0x54));
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x49f7bc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(ecx);
                m.write32(esp.wrapping_add(0x54), (fpu.st(0) as f32).to_bits()); fpu.pop();
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x24))) as f64));
                edx = m.read32(esp.wrapping_add(0x54));
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x49f7bc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(edx);
                m.write32(esp.wrapping_add(0x3c), (fpu.st(0) as f32).to_bits()); fpu.pop();
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x2c))) as f64));
                eax = m.read32(esp.wrapping_add(0x3c));
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475c78)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(eax);
                m.write32(esp.wrapping_add(0x48), (fpu.st(0) as f32).to_bits()); fpu.pop();
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x2c))) as f64));
                ecx = m.read32(esp.wrapping_add(0x48));
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(0x475dc4)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x30));
                m.write32(esp.wrapping_add(0x48), (fpu.st(0) as f32).to_bits()); fpu.pop();
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x34))) as f64));
                edx = m.read32(esp.wrapping_add(0x48));
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475dc4)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(edx);
                m.write32(esp.wrapping_add(0x60), (fpu.st(0) as f32).to_bits()); fpu.pop();
                eax = m.read32(esp.wrapping_add(0x60));
                push!(eax);
                push!(ecx);
                push!(ecx);
                m.write32(esp, xmm0.lanes[0]);
                call!(0x308c00);
                pc = 0x30f1b0; continue;
            }
            0x30f1b0 => {
                xh[0] = Xmm::scalar(m.read32(0x783dd8));
                xmm0.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x44)));
                xh[1] = Xmm::scalar(m.read32(0x783f2c));
                xmm3 = Xmm::scalar(m.read32(esp.wrapping_add(0x6c)));
                xh[2] = Xmm::scalar(m.read32(0x783cc0));
                xmm4.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x58)));
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x40)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475dc4))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = Xmm::scalar(m.read32(0x475c78));
                m.write32(esp.wrapping_add(0xa8), xmm0.lanes[0]);
                esp = esp.wrapping_add(0x30);
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0xdc), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0xd4), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x70), xmm3.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30f26f; continue; }
                pc = 0x30f265; continue;
            }
            0x30f265 => {
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x78), xmm1.lanes[0]);
                pc = 0x30f26f; continue;
            }
            0x30f26f => {
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm3.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30f282; continue; }
                pc = 0x30f278; continue;
            }
            0x30f278 => {
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x70), xmm2.lanes[0]);
                pc = 0x30f282; continue;
            }
            0x30f282 => {
                edx = m.read32(esp.wrapping_add(0x78));
                eax = m.read32(esp.wrapping_add(0x70));
                ecx = m.read32(esp.wrapping_add(0xd4));
                push!(ebp);
                push!(edx);
                edx = m.read32(esp.wrapping_add(0xe4));
                push!(eax);
                push!(ecx);
                push!(edx);
                call!(0x1a2e0);
                pc = 0x30f2a2; continue;
            }
            0x30f2a2 => {
                xh[0] = Xmm::scalar(m.read32(0x783dd8));
                xmm0.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x40)));
                xh[1] = Xmm::scalar(m.read32(0x783f2c));
                xmm3 = Xmm::scalar(m.read32(esp.wrapping_add(0x54)));
                xh[2] = Xmm::scalar(m.read32(0x783cc0));
                xmm4.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x58)));
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x38)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = Xmm::scalar(m.read32(0x475c78));
                m.write32(esp.wrapping_add(0x88), xmm0.lanes[0]);
                esp = esp.wrapping_add(0x14);
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0xec), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0xe4), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x80), xmm3.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30f35c; continue; }
                pc = 0x30f352; continue;
            }
            0x30f352 => {
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x74), xmm1.lanes[0]);
                pc = 0x30f35c; continue;
            }
            0x30f35c => {
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm3.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30f372; continue; }
                pc = 0x30f365; continue;
            }
            0x30f365 => {
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x80), xmm2.lanes[0]);
                pc = 0x30f372; continue;
            }
            0x30f372 => {
                eax = m.read32(esp.wrapping_add(0x74));
                ecx = m.read32(esp.wrapping_add(0x80));
                edx = m.read32(esp.wrapping_add(0xe4));
                push!(ebp);
                push!(eax);
                eax = m.read32(esp.wrapping_add(0xf4));
                push!(ecx);
                push!(edx);
                push!(eax);
                call!(0x1a2e0);
                pc = 0x30f395; continue;
            }
            0x30f395 => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x28)));
                edx = m.read32(esp.wrapping_add(0x30));
                esp = esp.wrapping_add(0x14);
                pc = 0x30f3a8; continue;
            }
            0x30f3a8 => {
                fa = edx;
                fb = ((((0x200000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) { pc = 0x30f644; continue; }
                pc = 0x30f3b4; continue;
            }
            0x30f3b4 => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x10))) as f64));
                push!(ebp);
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x49f7bc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(ebp);
                push!(ebp);
                push!(ebp);
                m.write32(esp.wrapping_add(0x34), (fpu.st(0) as f32).to_bits()); fpu.pop();
                xmm2 = xmm0;
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x24))) as f64));
                ecx = m.read32(esp.wrapping_add(0x34));
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475dc4)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                xmm0 = Xmm::scalar(m.read32(0x475dc4));
                push!(ecx);
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x50), (fpu.st(0) as f32).to_bits()); fpu.pop();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x24))) as f64));
                edx = m.read32(esp.wrapping_add(0x50));
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475c78)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(edx);
                m.write32(esp.wrapping_add(0x40), (fpu.st(0) as f32).to_bits()); fpu.pop();
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x28))) as f64));
                eax = m.read32(esp.wrapping_add(0x40));
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475dc4)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(eax);
                push!(ecx);
                m.write32(esp, xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0x64), (fpu.st(0) as f32).to_bits()); fpu.pop();
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x34))) as f64));
                ecx = m.read32(esp.wrapping_add(0x64));
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x49f7bc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(ecx);
                m.write32(esp.wrapping_add(0x64), (fpu.st(0) as f32).to_bits()); fpu.pop();
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x38))) as f64));
                edx = m.read32(esp.wrapping_add(0x64));
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475c78)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(edx);
                push!(ecx);
                m.write32(esp, xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x58), (fpu.st(0) as f32).to_bits()); fpu.pop();
                eax = m.read32(esp.wrapping_add(0x58));
                push!(eax);
                call!(0x308c00);
                pc = 0x30f44e; continue;
            }
            0x30f44e => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x5c)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x475dc4))).to_bits();
                xh[0] = Xmm::scalar(m.read32(0x783dd8));
                xmm0.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xh[1] = Xmm::scalar(m.read32(0x783f2c));
                xmm3 = Xmm::scalar(m.read32(esp.wrapping_add(0x70)));
                xh[2] = Xmm::scalar(m.read32(0x783cc0));
                xmm4.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x40)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x74)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = Xmm::scalar(m.read32(0x475c78));
                m.write32(esp.wrapping_add(0xb8), xmm0.lanes[0]);
                esp = esp.wrapping_add(0x30);
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0xfc), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0xf4), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0x90), xmm3.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30f513; continue; }
                pc = 0x30f506; continue;
            }
            0x30f506 => {
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x88), xmm2.lanes[0]);
                pc = 0x30f513; continue;
            }
            0x30f513 => {
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm3.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30f529; continue; }
                pc = 0x30f51c; continue;
            }
            0x30f51c => {
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x90), xmm1.lanes[0]);
                pc = 0x30f529; continue;
            }
            0x30f529 => {
                ecx = m.read32(esp.wrapping_add(0x88));
                edx = m.read32(esp.wrapping_add(0x90));
                eax = m.read32(esp.wrapping_add(0xf4));
                push!(ebp);
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x104));
                push!(edx);
                push!(eax);
                push!(ecx);
                call!(0x1a2e0);
                pc = 0x30f54f; continue;
            }
            0x30f54f => {
                xh[0] = Xmm::scalar(m.read32(0x783dd8));
                xmm3.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x28)));
                xh[1] = Xmm::scalar(m.read32(0x783f2c));
                xh[2] = Xmm::scalar(m.read32(0x783cc0));
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x3c)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x475dc4))).to_bits();
                xmm4.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x50)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x34), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x38)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = Xmm::scalar(m.read32(0x475c78));
                m.write32(esp.wrapping_add(0x90), xmm0.lanes[0]);
                esp = esp.wrapping_add(0x14);
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                fca = f32::from_bits(xmm3.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x38), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x34), xmm2.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30f608; continue; }
                pc = 0x30f5fe; continue;
            }
            0x30f5fe => {
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x7c), xmm1.lanes[0]);
                pc = 0x30f608; continue;
            }
            0x30f608 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x20)));
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                fca = f32::from_bits(xmm3.lanes[0]) as f64; fcb = f32::from_bits(xmm2.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30f621; continue; }
                pc = 0x30f617; continue;
            }
            0x30f617 => {
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x34), xmm0.lanes[0]);
                pc = 0x30f621; continue;
            }
            0x30f621 => {
                edx = m.read32(esp.wrapping_add(0x7c));
                eax = m.read32(esp.wrapping_add(0x34));
                ecx = m.read32(esp.wrapping_add(0x38));
                push!(ebp);
                push!(edx);
                edx = m.read32(esp.wrapping_add(0x28));
                push!(eax);
                push!(ecx);
                push!(edx);
                call!(0x1a2e0);
                pc = 0x30f63b; continue;
            }
            0x30f63b => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                esp = esp.wrapping_add(0x14);
                pc = 0x30f644; continue;
            }
            0x30f644 => {
                eax = m.read32(edi.wrapping_add(0x14));
                esi = esi.wrapping_add(1);
                fa = esi;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x30e790; continue; }
                pc = 0x30f650; continue;
            }
            0x30f650 => {
                eax = m.read32(esp.wrapping_add(0x18));
                ecx = m.read32(edi.wrapping_add(0x18));
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x18), eax);
                if (fas < fbs) { pc = 0x30e766; continue; }
                pc = 0x30f664; continue;
            }
            0x30f664 => {
                eax = m.read32(esp.wrapping_add(0x124));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30ff29; continue; }
                pc = 0x30f673; continue;
            }
            0x30f673 => {
                eax = m.read32(esp.wrapping_add(0x114));
                ecx = m.read32(eax.wrapping_add(0x44));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x34), 0);
                if (fas <= fbs) { pc = 0x30ff29; continue; }
                pc = 0x30f68d; continue;
            }
            0x30f68d => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x120)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475cd4))).to_bits();
                esi = eax;
                m.write32(esp.wrapping_add(0x28), xmm0.lanes[0]);
                esi = esi.wrapping_add(0x4c);
                pc = 0x30f6b0; continue;
            }
            0x30f6b0 => {
                ecx = m.read32(0x7844a8);
                fpu.push((m.read32(ecx.wrapping_add(0x264)) as i32 as f64));
                edx = m.read32(esp.wrapping_add(0x11c));
                m.write32(esp.wrapping_add(0x20), (fpu.st(0) as f32).to_bits());
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(edx.wrapping_add(0x8))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                call!(0x3c848c);
                pc = 0x30f6cf; continue;
            }
            0x30f6cf => {
                ecx = ((m.read8(esi.wrapping_add(((0x1i32.wrapping_neg()) as u32)))) as u8 as u32);
                fa = eax;
                fb = ecx;
                if (fa != fb) { pc = 0x30ff0b; continue; }
                pc = 0x30f6db; continue;
            }
            0x30f6db => {
                fa = ((((m.read8(esi.wrapping_add(0x1)))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x30ff0b; continue; }
                pc = 0x30f6e5; continue;
            }
            0x30f6e5 => {
                eax = ((m.read8(esi.wrapping_add(((0x2i32.wrapping_neg()) as u32)))) as u8 as u32);
                edx = ((m.read8(esi.wrapping_add(((0x3i32.wrapping_neg()) as u32)))) as u8 as u32);
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x20)));
                xmm1.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x110)))).to_bits();
                eax = m.read32(esp.wrapping_add(0x118));
                fa = eax;
                fb = ((((0x9i32)) as u32) & 0xffffffffu32);
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x10c)))).to_bits();
                ebx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                ebp = sse::cvttss2si(f32::from_bits(xmm1.lanes[0]));
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x14), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x18), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0x24), ebx);
                m.write32(esp.wrapping_add(0x20), ebp);
                if (fa != fb) { pc = 0x30f747; continue; }
                pc = 0x30f73b; continue;
            }
            0x30f73b => {
                ecx = m.read32(esp.wrapping_add(0x28));
                push!(ecx);
                push!(0x645f7f5a);
                pc = 0x30f773; continue;
            }
            0x30f747 => {
                fa = eax;
                fb = ((((0x8i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x30f758; continue; }
                pc = 0x30f74c; continue;
            }
            0x30f74c => {
                edx = m.read32(esp.wrapping_add(0x28));
                push!(edx);
                push!(0x645f7f78);
                pc = 0x30f773; continue;
            }
            0x30f758 => {
                fa = eax;
                fb = ((((0x6i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x30f769; continue; }
                pc = 0x30f75d; continue;
            }
            0x30f75d => {
                eax = m.read32(esp.wrapping_add(0x28));
                push!(eax);
                push!(0x645f7f32);
                pc = 0x30f773; continue;
            }
            0x30f769 => {
                ecx = m.read32(esp.wrapping_add(0x28));
                push!(ecx);
                push!(0x645f7f64);
                pc = 0x30f773; continue;
            }
            0x30f773 => {
                call!(0x154fe0);
                pc = 0x30f778; continue;
            }
            0x30f778 => {
                edi = eax;
                eax = (eax & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(0x1))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x8);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                m.write32(esp.wrapping_add(0x2c), edi);
                if (fa != fb) { pc = 0x30fc8a; continue; }
                pc = 0x30f78c; continue;
            }
            0x30f78c => {
                fa = ((((m.read8(esi))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if (fa <= fb) { pc = 0x30fc8a; continue; }
                pc = 0x30f795; continue;
            }
            0x30f795 => {
                eax = ((m.read8(esi.wrapping_add(((0x4i32.wrapping_neg()) as u32)))) as u8 as u32);
                eax = eax.wrapping_sub(1);
                fa = eax;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                if (fa > fb) { pc = 0x30ff0b; continue; }
                pc = 0x30f7a3; continue;
            }
            0x30f7a3 => {
                jt = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x3100dc));
                if (jt == 0x30f7aau32) { pc = 0x30f7aa; continue; }
                if (jt == 0x30f8e3u32) { pc = 0x30f8e3; continue; }
                if (jt == 0x30fa0du32) { pc = 0x30fa0d; continue; }
                if (jt == 0x30fb45u32) { pc = 0x30fb45; continue; }
                tail_to!(jt); break 'run;  // indirect tail jump, target outside the switch
            }
            0x30f7aa => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x14))) as f64));
                edx = m.read32(esp.wrapping_add(0x18));
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(0x475c78)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                ecx = m.read32(esp.wrapping_add(0x10));
                push!(edx);
                m.write32(esp.wrapping_add(0x24), (fpu.st(0) as f32).to_bits()); fpu.pop();
                eax = m.read32(esp.wrapping_add(0x24));
                push!(eax);
                push!(ecx);
                call!(0x2f8140);
                pc = 0x30f7cc; continue;
            }
            0x30f7cc => {
                esp = esp.wrapping_add(0xc);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x30f814; continue; }
                pc = 0x30f7d3; continue;
            }
            0x30f7d3 => {
                edx = ((m.read8(esi)) as u8 as u32);
                ecx = ((m.read8(esi.wrapping_add(((0x1i32.wrapping_neg()) as u32)))) as u8 as u32);
                eax = ((m.read8(esi.wrapping_add(((0x4i32.wrapping_neg()) as u32)))) as u8 as u32);
                push!(edx);
                edx = m.read32(esp.wrapping_add(0x120));
                push!(eax);
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x8)))).to_bits();
                ecx = m.read32(0x7844a8);
                edx = m.read32(ecx.wrapping_add(0x2f80));
                eax = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(eax);
                push!(ebp);
                push!(ebx);
                push!(edx);
                call!(0x2f8780);
                pc = 0x30f809; continue;
            }
            0x30f809 => {
                esp = esp.wrapping_add(0x18);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x30ff0b; continue; }
                pc = 0x30f814; continue;
            }
            0x30f814 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(esi)) as u32)) & 0xff);
                edi = 0;
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa <= fb) { pc = 0x30ff0b; continue; }
                pc = 0x30f820; continue;
            }
            0x30f820 => {
                eax = ((m.read8(esi.wrapping_add(((0x3i32.wrapping_neg()) as u32)))) as u8 as u32);
                ecx = m.read32(esp.wrapping_add(0x18));
                edx = m.read32(esp.wrapping_add(0x20));
                eax = eax.wrapping_add(edi);
                m.write32(esp.wrapping_add(0x38), eax);
                push!(ecx);
                push!(edx);
                fpu.push((m.read32(esp.wrapping_add(0x40)) as i32 as f64));
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(esp.wrapping_add(0x114))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x18), (fpu.st(0) as f32).to_bits()); fpu.pop();
                ebx = m.read32(esp.wrapping_add(0x18));
                push!(ebx);
                call!(0x2f8140);
                pc = 0x30f84d; continue;
            }
            0x30f84d => {
                esp = esp.wrapping_add(0xc);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x30f895; continue; }
                pc = 0x30f854; continue;
            }
            0x30f854 => {
                ecx = ((m.read8(esi.wrapping_add(((0x1i32.wrapping_neg()) as u32)))) as u8 as u32);
                eax = ((m.read8(esi.wrapping_add(((0x4i32.wrapping_neg()) as u32)))) as u8 as u32);
                edx = m.read32(esp.wrapping_add(0x11c));
                push!(0x1);
                push!(eax);
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x8)))).to_bits();
                edx = m.read32(0x7844a8);
                ecx = (((((f32::from_bits(m.read32(esp.wrapping_add(0x18))) as f64)) as i32)) as u32);
                eax = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(eax);
                eax = m.read32(edx.wrapping_add(0x2f80));
                push!(ebp);
                push!(ecx);
                push!(eax);
                call!(0x2f8780);
                pc = 0x30f88e; continue;
            }
            0x30f88e => {
                esp = esp.wrapping_add(0x18);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x30f8d2; continue; }
                pc = 0x30f895; continue;
            }
            0x30f895 => {
                ecx = m.read32(esp.wrapping_add(0x2c));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475dc4))).to_bits();
                edx = m.read32(esp.wrapping_add(0x14));
                push!(ecx);
                esp = esp.wrapping_sub(0x8);
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x1c)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475c78))).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                push!(edx);
                push!(ebx);
                call!(0x308b10);
                pc = 0x30f8cf; continue;
            }
            0x30f8cf => {
                esp = esp.wrapping_add(0x14);
                pc = 0x30f8d2; continue;
            }
            0x30f8d2 => {
                eax = ((m.read8(esi)) as u8 as u32);
                edi = edi.wrapping_add(1);
                fa = edi;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x30f820; continue; }
                pc = 0x30f8de; continue;
            }
            0x30f8de => {
                pc = 0x30ff0b; continue;
            }
            0x30f8e3 => {
                ecx = m.read32(esp.wrapping_add(0x18));
                edx = m.read32(esp.wrapping_add(0x14));
                eax = m.read32(esp.wrapping_add(0x10));
                push!(ecx);
                push!(edx);
                push!(eax);
                call!(0x2f8140);
                pc = 0x30f8f7; continue;
            }
            0x30f8f7 => {
                esp = esp.wrapping_add(0xc);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x8i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x30f93e; continue; }
                pc = 0x30f8fe; continue;
            }
            0x30f8fe => {
                ecx = ((m.read8(esi)) as u8 as u32);
                eax = ((m.read8(esi.wrapping_add(((0x1i32.wrapping_neg()) as u32)))) as u8 as u32);
                edx = ((m.read8(esi.wrapping_add(((0x4i32.wrapping_neg()) as u32)))) as u8 as u32);
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x120));
                push!(edx);
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x8)))).to_bits();
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f80));
                edx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(edx);
                push!(ebp);
                push!(ebx);
                push!(ecx);
                call!(0x2f8780);
                pc = 0x30f933; continue;
            }
            0x30f933 => {
                esp = esp.wrapping_add(0x18);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x30ff0b; continue; }
                pc = 0x30f93e; continue;
            }
            0x30f93e => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(esi)) as u32)) & 0xff);
                edi = 0;
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa <= fb) { pc = 0x30ff0b; continue; }
                pc = 0x30f94a; continue;
            }
            0x30f94a => {
                ebp = m.read32(esp.wrapping_add(0x10));
                edi = edi;
                pc = 0x30f950; continue;
            }
            0x30f950 => {
                edx = ((m.read8(esi.wrapping_add(((0x2i32.wrapping_neg()) as u32)))) as u8 as u32);
                eax = m.read32(esp.wrapping_add(0x18));
                edx = edx.wrapping_add(edi);
                m.write32(esp.wrapping_add(0x38), edx);
                push!(eax);
                fpu.push((m.read32(esp.wrapping_add(0x3c)) as i32 as f64));
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(esp.wrapping_add(0x114))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x18), (fpu.st(0) as f32).to_bits()); fpu.pop();
                ebx = m.read32(esp.wrapping_add(0x18));
                push!(ebx);
                push!(ebp);
                call!(0x2f8140);
                pc = 0x30f979; continue;
            }
            0x30f979 => {
                esp = esp.wrapping_add(0xc);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x8i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x30f9c5; continue; }
                pc = 0x30f980; continue;
            }
            0x30f980 => {
                edx = ((m.read8(esi.wrapping_add(((0x1i32.wrapping_neg()) as u32)))) as u8 as u32);
                ecx = ((m.read8(esi.wrapping_add(((0x4i32.wrapping_neg()) as u32)))) as u8 as u32);
                eax = m.read32(esp.wrapping_add(0x11c));
                push!(0x1);
                push!(ecx);
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x8)))).to_bits();
                edx = (((((f32::from_bits(m.read32(esp.wrapping_add(0x1c))) as f64)) as i32)) as u32);
                eax = m.read32(esp.wrapping_add(0x2c));
                ecx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(ecx);
                ecx = m.read32(0x7844a8);
                push!(edx);
                edx = m.read32(ecx.wrapping_add(0x2f80));
                push!(eax);
                push!(edx);
                call!(0x2f8780);
                pc = 0x30f9be; continue;
            }
            0x30f9be => {
                esp = esp.wrapping_add(0x18);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x30f9fc; continue; }
                pc = 0x30f9c5; continue;
            }
            0x30f9c5 => {
                eax = m.read32(esp.wrapping_add(0x2c));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475c78))).to_bits();
                push!(eax);
                push!(ecx);
                m.write32(esp, xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x18)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475dc4))).to_bits();
                push!(ebp);
                push!(ebx);
                push!(ecx);
                m.write32(esp, xmm0.lanes[0]);
                call!(0x308b10);
                pc = 0x30f9f9; continue;
            }
            0x30f9f9 => {
                esp = esp.wrapping_add(0x14);
                pc = 0x30f9fc; continue;
            }
            0x30f9fc => {
                ecx = ((m.read8(esi)) as u8 as u32);
                edi = edi.wrapping_add(1);
                fa = edi;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x30f950; continue; }
                pc = 0x30fa08; continue;
            }
            0x30fa08 => {
                pc = 0x30ff0b; continue;
            }
            0x30fa0d => {
                edx = m.read32(esp.wrapping_add(0x18));
                ebp = m.read32(esp.wrapping_add(0x14));
                eax = m.read32(esp.wrapping_add(0x10));
                push!(edx);
                push!(ebp);
                push!(eax);
                call!(0x2f8140);
                pc = 0x30fa21; continue;
            }
            0x30fa21 => {
                esp = esp.wrapping_add(0xc);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x30fa71; continue; }
                pc = 0x30fa28; continue;
            }
            0x30fa28 => {
                ecx = ((m.read8(esi)) as u8 as u32);
                eax = ((m.read8(esi.wrapping_add(((0x1i32.wrapping_neg()) as u32)))) as u8 as u32);
                edx = ((m.read8(esi.wrapping_add(((0x4i32.wrapping_neg()) as u32)))) as u8 as u32);
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x120));
                push!(edx);
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x8)))).to_bits();
                eax = m.read32(esp.wrapping_add(0x28));
                ecx = m.read32(esp.wrapping_add(0x2c));
                edx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(edx);
                edx = m.read32(0x7844a8);
                push!(eax);
                eax = m.read32(edx.wrapping_add(0x2f80));
                push!(ecx);
                push!(eax);
                call!(0x2f8780);
                pc = 0x30fa66; continue;
            }
            0x30fa66 => {
                esp = esp.wrapping_add(0x18);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x30ff0b; continue; }
                pc = 0x30fa71; continue;
            }
            0x30fa71 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(esi)) as u32)) & 0xff);
                edi = 0;
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa <= fb) { pc = 0x30ff0b; continue; }
                pc = 0x30fa7d; continue;
            }
            0x30fa7d => {
                pc = 0x30fa80; continue;
            }
            0x30fa80 => {
                ecx = ((m.read8(esi)) as u8 as u32);
                edx = ((m.read8(esi.wrapping_add(((0x3i32.wrapping_neg()) as u32)))) as u8 as u32);
                eax = m.read32(esp.wrapping_add(0x18));
                ecx = ecx.wrapping_add(edx);
                ecx = ecx.wrapping_sub(edi);
                ecx = ecx.wrapping_sub(1);
                m.write32(esp.wrapping_add(0x38), ecx);
                push!(eax);
                push!(ebp);
                fpu.push((m.read32(esp.wrapping_add(0x40)) as i32 as f64));
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(esp.wrapping_add(0x114))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x18), (fpu.st(0) as f32).to_bits()); fpu.pop();
                ebx = m.read32(esp.wrapping_add(0x18));
                push!(ebx);
                call!(0x2f8140);
                pc = 0x30faaf; continue;
            }
            0x30faaf => {
                esp = esp.wrapping_add(0xc);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x30fafb; continue; }
                pc = 0x30fab6; continue;
            }
            0x30fab6 => {
                edx = ((m.read8(esi.wrapping_add(((0x1i32.wrapping_neg()) as u32)))) as u8 as u32);
                ecx = ((m.read8(esi.wrapping_add(((0x4i32.wrapping_neg()) as u32)))) as u8 as u32);
                eax = m.read32(esp.wrapping_add(0x11c));
                push!(0x1);
                push!(ecx);
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x8)))).to_bits();
                edx = m.read32(esp.wrapping_add(0x28));
                eax = (((((f32::from_bits(m.read32(esp.wrapping_add(0x18))) as f64)) as i32)) as u32);
                ecx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(ecx);
                ecx = m.read32(0x7844a8);
                push!(edx);
                edx = m.read32(ecx.wrapping_add(0x2f80));
                push!(eax);
                push!(edx);
                call!(0x2f8780);
                pc = 0x30faf4; continue;
            }
            0x30faf4 => {
                esp = esp.wrapping_add(0x18);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x30fb34; continue; }
                pc = 0x30fafb; continue;
            }
            0x30fafb => {
                eax = m.read32(esp.wrapping_add(0x2c));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x10)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475c78))).to_bits();
                push!(eax);
                push!(ebp);
                esp = esp.wrapping_sub(0x8);
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475dc4))).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                push!(ebx);
                call!(0x308b10);
                pc = 0x30fb31; continue;
            }
            0x30fb31 => {
                esp = esp.wrapping_add(0x14);
                pc = 0x30fb34; continue;
            }
            0x30fb34 => {
                ecx = ((m.read8(esi)) as u8 as u32);
                edi = edi.wrapping_add(1);
                fa = edi;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x30fa80; continue; }
                pc = 0x30fb40; continue;
            }
            0x30fb40 => {
                pc = 0x30ff0b; continue;
            }
            0x30fb45 => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x10))) as f64));
                edx = m.read32(esp.wrapping_add(0x18));
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(0x475c78)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                eax = m.read32(esp.wrapping_add(0x14));
                push!(edx);
                push!(eax);
                m.write32(esp.wrapping_add(0x28), (fpu.st(0) as f32).to_bits()); fpu.pop();
                ecx = m.read32(esp.wrapping_add(0x28));
                push!(ecx);
                call!(0x2f8140);
                pc = 0x30fb67; continue;
            }
            0x30fb67 => {
                esp = esp.wrapping_add(0xc);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x30fbaf; continue; }
                pc = 0x30fb6e; continue;
            }
            0x30fb6e => {
                edx = ((m.read8(esi)) as u8 as u32);
                ecx = ((m.read8(esi.wrapping_add(((0x1i32.wrapping_neg()) as u32)))) as u8 as u32);
                eax = ((m.read8(esi.wrapping_add(((0x4i32.wrapping_neg()) as u32)))) as u8 as u32);
                push!(edx);
                edx = m.read32(esp.wrapping_add(0x120));
                push!(eax);
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x8)))).to_bits();
                ecx = m.read32(0x7844a8);
                edx = m.read32(ecx.wrapping_add(0x2f80));
                eax = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(eax);
                push!(ebp);
                push!(ebx);
                push!(edx);
                call!(0x2f8780);
                pc = 0x30fba4; continue;
            }
            0x30fba4 => {
                esp = esp.wrapping_add(0x18);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x30ff0b; continue; }
                pc = 0x30fbaf; continue;
            }
            0x30fbaf => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(esi)) as u32)) & 0xff);
                edi = 0;
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa <= fb) { pc = 0x30ff0b; continue; }
                pc = 0x30fbbb; continue;
            }
            0x30fbbb => {
                ebp = m.read32(esp.wrapping_add(0x18));
                pc = 0x30fbc0; continue;
            }
            0x30fbc0 => {
                eax = ((m.read8(esi)) as u8 as u32);
                ecx = ((m.read8(esi.wrapping_add(((0x2i32.wrapping_neg()) as u32)))) as u8 as u32);
                eax = eax.wrapping_sub(edi);
                edx = eax.wrapping_add(ecx).wrapping_add(((0x1i32.wrapping_neg()) as u32));
                eax = m.read32(esp.wrapping_add(0x20));
                m.write32(esp.wrapping_add(0x38), edx);
                push!(ebp);
                fpu.push((m.read32(esp.wrapping_add(0x3c)) as i32 as f64));
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(esp.wrapping_add(0x114))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x18), (fpu.st(0) as f32).to_bits()); fpu.pop();
                ebx = m.read32(esp.wrapping_add(0x18));
                push!(ebx);
                push!(eax);
                call!(0x2f8140);
                pc = 0x30fbf0; continue;
            }
            0x30fbf0 => {
                esp = esp.wrapping_add(0xc);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x30fc3c; continue; }
                pc = 0x30fbf7; continue;
            }
            0x30fbf7 => {
                edx = ((m.read8(esi.wrapping_add(((0x1i32.wrapping_neg()) as u32)))) as u8 as u32);
                ecx = ((m.read8(esi.wrapping_add(((0x4i32.wrapping_neg()) as u32)))) as u8 as u32);
                eax = m.read32(esp.wrapping_add(0x11c));
                push!(0x1);
                push!(ecx);
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x8)))).to_bits();
                edx = (((((f32::from_bits(m.read32(esp.wrapping_add(0x1c))) as f64)) as i32)) as u32);
                eax = m.read32(esp.wrapping_add(0x2c));
                ecx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(ecx);
                ecx = m.read32(0x7844a8);
                push!(edx);
                edx = m.read32(ecx.wrapping_add(0x2f80));
                push!(eax);
                push!(edx);
                call!(0x2f8780);
                pc = 0x30fc35; continue;
            }
            0x30fc35 => {
                esp = esp.wrapping_add(0x18);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x30fc79; continue; }
                pc = 0x30fc3c; continue;
            }
            0x30fc3c => {
                eax = m.read32(esp.wrapping_add(0x2c));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475c78))).to_bits();
                ecx = m.read32(esp.wrapping_add(0x10));
                push!(eax);
                esp = esp.wrapping_sub(0x8);
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x1c)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475dc4))).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                push!(ebx);
                push!(ecx);
                call!(0x308b10);
                pc = 0x30fc76; continue;
            }
            0x30fc76 => {
                esp = esp.wrapping_add(0x14);
                pc = 0x30fc79; continue;
            }
            0x30fc79 => {
                edx = ((m.read8(esi)) as u8 as u32);
                edi = edi.wrapping_add(1);
                fa = edi;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x30fbc0; continue; }
                pc = 0x30fc85; continue;
            }
            0x30fc85 => {
                pc = 0x30ff0b; continue;
            }
            0x30fc8a => {
                eax = ((m.read8(esi.wrapping_add(((0x4i32.wrapping_neg()) as u32)))) as u8 as u32);
                eax = eax.wrapping_sub(1);
                fa = eax;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                if (fa > fb) { pc = 0x30ff0b; continue; }
                pc = 0x30fc98; continue;
            }
            0x30fc98 => {
                jt = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x3100ec));
                if (jt == 0x30fc9fu32) { pc = 0x30fc9f; continue; }
                if (jt == 0x30fd44u32) { pc = 0x30fd44; continue; }
                if (jt == 0x30fdd8u32) { pc = 0x30fdd8; continue; }
                if (jt == 0x30fe68u32) { pc = 0x30fe68; continue; }
                tail_to!(jt); break 'run;  // indirect tail jump, target outside the switch
            }
            0x30fc9f => {
                eax = m.read32(esp.wrapping_add(0x18));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475c78))).to_bits();
                push!(eax);
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x18));
                m.write32(esp, xmm0.lanes[0]);
                push!(ecx);
                call!(0x2f8140);
                pc = 0x30fcc2; continue;
            }
            0x30fcc2 => {
                esp = esp.wrapping_add(0xc);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x30fd0b; continue; }
                pc = 0x30fcc9; continue;
            }
            0x30fcc9 => {
                edx = ((m.read8(esi)) as u8 as u32);
                ecx = ((m.read8(esi.wrapping_add(((0x1i32.wrapping_neg()) as u32)))) as u8 as u32);
                eax = ((m.read8(esi.wrapping_add(((0x4i32.wrapping_neg()) as u32)))) as u8 as u32);
                push!(edx);
                edx = m.read32(esp.wrapping_add(0x120));
                push!(eax);
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x8)))).to_bits();
                ecx = m.read32(0x7844a8);
                edx = m.read32(ecx.wrapping_add(0x2f80));
                eax = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(eax);
                push!(ebp);
                push!(ebx);
                push!(edx);
                call!(0x2f8780);
                pc = 0x30fcff; continue;
            }
            0x30fcff => {
                esp = esp.wrapping_add(0x18);
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x30ff0b; continue; }
                pc = 0x30fd0b; continue;
            }
            0x30fd0b => {
                eax = ((m.read8(esi)) as u8 as u32);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475dc4))).to_bits();
                ecx = m.read32(esp.wrapping_add(0x14));
                edx = m.read32(esp.wrapping_add(0x10));
                push!(edi);
                esp = esp.wrapping_sub(0x8);
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x1c)))).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                push!(ecx);
                push!(edx);
                pc = 0x30ff03; continue;
            }
            0x30fd44 => {
                eax = m.read32(esp.wrapping_add(0x18));
                ecx = m.read32(esp.wrapping_add(0x14));
                edx = m.read32(esp.wrapping_add(0x10));
                push!(eax);
                push!(ecx);
                push!(edx);
                call!(0x2f8140);
                pc = 0x30fd58; continue;
            }
            0x30fd58 => {
                esp = esp.wrapping_add(0xc);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x8i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x30fda1; continue; }
                pc = 0x30fd5f; continue;
            }
            0x30fd5f => {
                eax = ((m.read8(esi)) as u8 as u32);
                edx = ((m.read8(esi.wrapping_add(((0x1i32.wrapping_neg()) as u32)))) as u8 as u32);
                ecx = ((m.read8(esi.wrapping_add(((0x4i32.wrapping_neg()) as u32)))) as u8 as u32);
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x120));
                push!(ecx);
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x8)))).to_bits();
                edx = m.read32(0x7844a8);
                eax = m.read32(edx.wrapping_add(0x2f80));
                ecx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(ecx);
                push!(ebp);
                push!(ebx);
                push!(eax);
                call!(0x2f8780);
                pc = 0x30fd95; continue;
            }
            0x30fd95 => {
                esp = esp.wrapping_add(0x18);
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x30ff0b; continue; }
                pc = 0x30fda1; continue;
            }
            0x30fda1 => {
                ecx = ((m.read8(esi)) as u8 as u32);
                edx = m.read32(esp.wrapping_add(0x10));
                eax = m.read32(esp.wrapping_add(0x14));
                push!(edi);
                push!(ecx);
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x1c)))).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x18)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475dc4))).to_bits();
                push!(edx);
                push!(eax);
                push!(ecx);
                m.write32(esp, xmm0.lanes[0]);
                pc = 0x30ff03; continue;
            }
            0x30fdd8 => {
                ecx = m.read32(esp.wrapping_add(0x18));
                edx = m.read32(esp.wrapping_add(0x14));
                eax = m.read32(esp.wrapping_add(0x10));
                push!(ecx);
                push!(edx);
                push!(eax);
                call!(0x2f8140);
                pc = 0x30fdec; continue;
            }
            0x30fdec => {
                esp = esp.wrapping_add(0xc);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x30fe34; continue; }
                pc = 0x30fdf3; continue;
            }
            0x30fdf3 => {
                ecx = ((m.read8(esi)) as u8 as u32);
                eax = ((m.read8(esi.wrapping_add(((0x1i32.wrapping_neg()) as u32)))) as u8 as u32);
                edx = ((m.read8(esi.wrapping_add(((0x4i32.wrapping_neg()) as u32)))) as u8 as u32);
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x120));
                push!(edx);
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x8)))).to_bits();
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f80));
                edx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(edx);
                push!(ebp);
                push!(ebx);
                push!(ecx);
                call!(0x2f8780);
                pc = 0x30fe28; continue;
            }
            0x30fe28 => {
                esp = esp.wrapping_add(0x18);
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x30ff0b; continue; }
                pc = 0x30fe34; continue;
            }
            0x30fe34 => {
                edx = m.read32(esp.wrapping_add(0x14));
                eax = ((m.read8(esi)) as u8 as u32);
                push!(edi);
                push!(edx);
                esp = esp.wrapping_sub(0x8);
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x20)))).to_bits();
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475dc4))).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                pc = 0x30fefe; continue;
            }
            0x30fe68 => {
                edx = m.read32(esp.wrapping_add(0x18));
                eax = m.read32(esp.wrapping_add(0x14));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x10)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475c78))).to_bits();
                push!(edx);
                push!(eax);
                push!(ecx);
                m.write32(esp, xmm0.lanes[0]);
                call!(0x2f8140);
                pc = 0x30fe8b; continue;
            }
            0x30fe8b => {
                esp = esp.wrapping_add(0xc);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x30fecf; continue; }
                pc = 0x30fe92; continue;
            }
            0x30fe92 => {
                ecx = ((m.read8(esi)) as u8 as u32);
                eax = ((m.read8(esi.wrapping_add(((0x1i32.wrapping_neg()) as u32)))) as u8 as u32);
                edx = ((m.read8(esi.wrapping_add(((0x4i32.wrapping_neg()) as u32)))) as u8 as u32);
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x120));
                push!(edx);
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x8)))).to_bits();
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f80));
                edx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(edx);
                push!(ebp);
                push!(ebx);
                push!(ecx);
                call!(0x2f8780);
                pc = 0x30fec7; continue;
            }
            0x30fec7 => {
                esp = esp.wrapping_add(0x18);
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x30ff0b; continue; }
                pc = 0x30fecf; continue;
            }
            0x30fecf => {
                edx = ((m.read8(esi)) as u8 as u32);
                eax = m.read32(esp.wrapping_add(0x14));
                push!(edi);
                esp = esp.wrapping_sub(0x8);
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x20)))).to_bits();
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x1c)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475dc4))).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                push!(eax);
                pc = 0x30fefe; continue;
            }
            0x30fefe => {
                ecx = m.read32(esp.wrapping_add(0x20));
                push!(ecx);
                pc = 0x30ff03; continue;
            }
            0x30ff03 => {
                call!(0x308b10);
                pc = 0x30ff08; continue;
            }
            0x30ff08 => {
                esp = esp.wrapping_add(0x14);
                pc = 0x30ff0b; continue;
            }
            0x30ff0b => {
                eax = m.read32(esp.wrapping_add(0x34));
                edx = m.read32(esp.wrapping_add(0x114));
                ecx = m.read32(edx.wrapping_add(0x44));
                eax = eax.wrapping_add(1);
                esi = esi.wrapping_add(0x6);
                fa = eax;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x34), eax);
                if (fas < fbs) { pc = 0x30f6b0; continue; }
                pc = 0x30ff29; continue;
            }
            0x30ff29 => {
                eax = m.read32(esp.wrapping_add(0x118));
                fa = eax;
                fb = ((((0x8i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3100ad; continue; }
                pc = 0x30ff39; continue;
            }
            0x30ff39 => {
                fa = eax;
                fb = ((((0x9i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3100ad; continue; }
                pc = 0x30ff42; continue;
            }
            0x30ff42 => {
                fa = eax;
                fb = ((((0xai32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3100ad; continue; }
                pc = 0x30ff4b; continue;
            }
            0x30ff4b => {
                edx = 0x6;
                fa = eax;
                fb = edx;
                if (fa == fb) { pc = 0x3100ad; continue; }
                pc = 0x30ff58; continue;
            }
            0x30ff58 => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x254));
                fa = eax;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30ff7b; continue; }
                pc = 0x30ff69; continue;
            }
            0x30ff69 => {
                fa = eax;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x30ff76; continue; }
                pc = 0x30ff6e; continue;
            }
            0x30ff6e => {
                fa = m.read32(ecx.wrapping_add(0x1185c));
                fb = edx;
                if (fa != fb) { pc = 0x30ff7b; continue; }
                pc = 0x30ff76; continue;
            }
            0x30ff76 => {
                fa = eax;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x30ff96; continue; }
                pc = 0x30ff7b; continue;
            }
            0x30ff7b => {
                eax = m.read32(esp.wrapping_add(0x11c));
                push!(eax);
                call!(0x30dd40);
                pc = 0x30ff88; continue;
            }
            0x30ff88 => {
                esp = esp.wrapping_add(0x4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0xf8);
                break 'run;
            }
            0x30ff96 => {
                fa = eax;
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30ffb0; continue; }
                pc = 0x30ff9b; continue;
            }
            0x30ff9b => {
                fa = eax;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3100ad; continue; }
                pc = 0x30ffa4; continue;
            }
            0x30ffa4 => {
                fa = m.read32(ecx.wrapping_add(0x1185c));
                fb = edx;
                if (fa != fb) { pc = 0x3100ad; continue; }
                pc = 0x30ffb0; continue;
            }
            0x30ffb0 => {
                eax = m.read32(esp.wrapping_add(0x114));
                xmm1 = Xmm::scalar(m.read32(0x475cd4));
                xmm0.lanes[0] = (((m.read32(eax.wrapping_add(0x14))) as i32) as f32).to_bits();
                xmm2 = Xmm::scalar(m.read32(0x475d4c));
                ecx = m.read32(esp.wrapping_add(0x11c));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x10c)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                xmm0.lanes[0] = (((m.read32(eax.wrapping_add(0x18))) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x110)))).to_bits();
                ecx = ecx.wrapping_add(0x18);
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                push!(ecx);
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                call!(0x30a2d0);
                pc = 0x31000f; continue;
            }
            0x31000f => {
                xmm1 = Xmm::scalar(m.read32(0x475cd4));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x2c)));
                ecx = m.read32(esp.wrapping_add(0x2c));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x28)));
                edx = m.read32(esp.wrapping_add(0x24));
                push!(0x78787880);
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                push!(edx);
                edx = m.read32(esp.wrapping_add(0x30));
                m.write32(esp.wrapping_add(0x40), xmm0.lanes[0]);
                esi = eax;
                eax = m.read32(esp.wrapping_add(0x40));
                push!(eax);
                push!(ecx);
                push!(edx);
                call!(0x308b10);
                pc = 0x310057; continue;
            }
            0x310057 => {
                xmm0 = Xmm::scalar(m.read32(0x475dc4));
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x38)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                esp = esp.wrapping_add(0x18);
                push!(esi);
                esp = esp.wrapping_sub(0x10);
                m.write32(esp.wrapping_add(0xc), xmm1.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x48)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x8), xmm1.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x3c)));
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x4), xmm1.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x38)));
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp, xmm1.lanes[0]);
                call!(0x308b10);
                pc = 0x3100aa; continue;
            }
            0x3100aa => {
                esp = esp.wrapping_add(0x14);
                pc = 0x3100ad; continue;
            }
            0x3100ad => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0xf8);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00310100: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f310100<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x310100;
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
        { crate::pose::cover(0x310100, previous, pc); previous = pc; }
        match pc {
            0x310100 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x254));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                push!(ebp);
                push!(esi);
                push!(edi);
                edi = m.read32(eax.wrapping_add(0x264));
                if (fa == fb) { pc = 0x31012f; continue; }
                pc = 0x310118; continue;
            }
            0x310118 => {
                fa = ecx;
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31012f; continue; }
                pc = 0x31011d; continue;
            }
            0x31011d => {
                fa = m.read32(eax.wrapping_add(0x1185c));
                fb = ((((0x6i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31012b; continue; }
                pc = 0x310126; continue;
            }
            0x310126 => {
                fa = ecx;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31012f; continue; }
                pc = 0x31012b; continue;
            }
            0x31012b => {
                ebp = 0;
                pc = 0x310134; continue;
            }
            0x31012f => {
                ebp = 0x1;
                pc = 0x310134; continue;
            }
            0x310134 => {
                ecx = m.read32(eax.wrapping_add(0x268));
                m.write32(eax.wrapping_add(0x264), ecx);
                esi = m.read32(0x78400c);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3101e0; continue; }
                pc = 0x31014e; continue;
            }
            0x31014e => {
                edi = edi;
                pc = 0x310150; continue;
            }
            0x310150 => {
                fpu.push((f32::from_bits(m.read32(esi.wrapping_add(0x8))) as f64));
                call!(0x3c848c);
                pc = 0x310158; continue;
            }
            0x310158 => {
                edx = edi;
                edx = edx.wrapping_sub(eax);
                eax = m.read32(esi.wrapping_add(0x2c));
                fa = edx;
                fb = m.read32(eax.wrapping_add(0x1c));
                if (fa != fb) { pc = 0x3101d5; continue; }
                pc = 0x310164; continue;
            }
            0x310164 => {
                xmm1 = Xmm::scalar(m.read32(0x783de0));
                xmm0 = Xmm::scalar(m.read32(0x783dd8));
                ecx = m.read32(esi.wrapping_add(0xc));
                eax = m.read32(esi.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x2c));
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esi)) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x3101d5; continue; }
                pc = 0x310184; continue;
            }
            0x310184 => {
                xmm2 = Xmm::scalar(m.read32(0x783ddc));
                xmm1 = Xmm::scalar(m.read32(0x783cc0));
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esi.wrapping_add(0x4))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x3101d5; continue; }
                pc = 0x31019e; continue;
            }
            0x31019e => {
                xmm2.lanes[0] = (((m.read32(eax.wrapping_add(0x14))) as i32) as f32).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esi))).to_bits();
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x3101d5; continue; }
                pc = 0x3101ac; continue;
            }
            0x3101ac => {
                xmm0.lanes[0] = (((m.read32(eax.wrapping_add(0x18))) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x4)))).to_bits();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x3101d5; continue; }
                pc = 0x3101bb; continue;
            }
            0x3101bb => {
                edx = m.read32(esi.wrapping_add(0x4));
                push!(0x1);
                push!(0x3f800000);
                push!(esi);
                push!(0x6);
                push!(eax);
                eax = m.read32(esi);
                push!(edx);
                push!(eax);
                call!(0x30de40);
                pc = 0x3101d2; continue;
            }
            0x3101d2 => {
                esp = esp.wrapping_add(0x1c);
                pc = 0x3101d5; continue;
            }
            0x3101d5 => {
                esi = m.read32(esi.wrapping_add(0x40));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x310150; continue; }
                pc = 0x3101e0; continue;
            }
            0x3101e0 => {
                ecx = m.read32(0x7844a8);
                m.write32(ecx.wrapping_add(0x264), edi);
                eax = m.read32(0x7844a8);
                m.write32(eax.wrapping_add(0x25c), (m.read32(eax.wrapping_add(0x25c)) | 0x800000));
                esi = m.read32(0x78400c);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                edi = m.read32(0x7844a8);
                if (fa == fb) { pc = 0x3102ce; continue; }
                pc = 0x31020f; continue;
            }
            0x31020f => {
                pc = 0x310210; continue;
            }
            0x310210 => {
                fpu.push((f32::from_bits(m.read32(esi.wrapping_add(0x8))) as f64));
                call!(0x3c848c);
                pc = 0x310218; continue;
            }
            0x310218 => {
                edx = m.read32(edi.wrapping_add(0x264));
                edx = edx.wrapping_sub(eax);
                eax = m.read32(esi.wrapping_add(0x2c));
                fa = edx;
                fb = m.read32(eax.wrapping_add(0x1c));
                if (fa >= fb) { pc = 0x3102c3; continue; }
                pc = 0x31022c; continue;
            }
            0x31022c => {
                xmm1 = Xmm::scalar(m.read32(0x783de0));
                xmm0 = Xmm::scalar(m.read32(0x783dd8));
                ecx = m.read32(esi.wrapping_add(0xc));
                ecx = m.read32(esi.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x2c));
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esi)) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x3102c3; continue; }
                pc = 0x31024c; continue;
            }
            0x31024c => {
                xmm2 = Xmm::scalar(m.read32(0x783ddc));
                xmm1 = Xmm::scalar(m.read32(0x783cc0));
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esi.wrapping_add(0x4))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x3102c3; continue; }
                pc = 0x310266; continue;
            }
            0x310266 => {
                xmm2.lanes[0] = (((m.read32(ecx.wrapping_add(0x14))) as i32) as f32).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esi))).to_bits();
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x3102c3; continue; }
                pc = 0x310274; continue;
            }
            0x310274 => {
                xmm0.lanes[0] = (((m.read32(ecx.wrapping_add(0x18))) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x4)))).to_bits();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x3102c3; continue; }
                pc = 0x310283; continue;
            }
            0x310283 => {
                edx = m.read32(edi.wrapping_add(0x2f18));
                eax = 0;
                fa = esi;
                fb = edx;
                if (fa != fb) { pc = 0x31029b; continue; }
                pc = 0x31028f; continue;
            }
            0x31028f => {
                eax = ebp;
                cf = (((((eax) != 0))) as i32);
                eax = (((((eax) as i32).wrapping_neg())) as u32);
                eax = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                eax = (eax & 0xfffffffau32);
                eax = eax.wrapping_add(0x7);
                pc = 0x31029b; continue;
            }
            0x31029b => {
                fa = ((((m.read8(esi.wrapping_add(0x14)))) as u32) & 0xffu32);
                fb = ((((0xai32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x3102a4; continue; }
                pc = 0x3102a1; continue;
            }
            0x3102a1 => {
                eax = (eax | 0x4);
                pc = 0x3102a4; continue;
            }
            0x3102a4 => {
                edx = m.read32(esi.wrapping_add(0x4));
                push!(0x1);
                push!(0x3f800000);
                push!(esi);
                push!(eax);
                eax = m.read32(esi);
                push!(ecx);
                push!(edx);
                push!(eax);
                call!(0x30de40);
                pc = 0x3102ba; continue;
            }
            0x3102ba => {
                edi = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x1c);
                pc = 0x3102c3; continue;
            }
            0x3102c3 => {
                esi = m.read32(esi.wrapping_add(0x40));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x310210; continue; }
                pc = 0x3102ce; continue;
            }
            0x3102ce => {
                fa = m.read32(edi.wrapping_add(0x254));
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3102f7; continue; }
                pc = 0x3102d7; continue;
            }
            0x3102d7 => {
                call!(0x30a250);
                pc = 0x3102dc; continue;
            }
            0x3102dc => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x25c));
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = (ecx & 0xff7fffffu32);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(eax.wrapping_add(0x25c), ecx);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x3102f7 => {
                m.write32(edi.wrapping_add(0x25c), (m.read32(edi.wrapping_add(0x25c)) & 0xff7fffffu32));
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00310310: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f310310<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x310310;
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
        { crate::pose::cover(0x310310, previous, pc); previous = pc; }
        match pc {
            0x310310 => {
                esp = esp.wrapping_sub(0x4c);
                eax = m.read32(0x7844a8);
                push!(edi);
                edi = 0;
                fa = ((((m.read16(eax.wrapping_add(0x306e)))) as u32) & 0xffffu32);
                fb = ((((((edi) as u16))) as u32) & 0xffffu32);
                m.write32(esp.wrapping_add(0x40), edi);
                if (fa <= fb) { pc = 0x3108c2; continue; }
                pc = 0x31032c; continue;
            }
            0x31032c => {
                push!(ebx);
                push!(ebp);
                push!(esi);
                pc = 0x310330; continue;
            }
            0x310330 => {
                edx = (((((m.read16(edi.wrapping_add(eax).wrapping_add(0x3072)) as i16)) as i32)) as u32);
                ecx = (((((m.read16(edi.wrapping_add(eax).wrapping_add(0x3070)) as i16)) as i32)) as u32);
                xmm3 = Xmm::scalar(m.read32(0x475d50));
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                edx = (edx & 0xffff_ff00) | ((((m.read8(edi.wrapping_add(eax).wrapping_add(0x3076))) as u32)) & 0xff);
                ebx = (ebx & 0xffff_ff00) | ((((((edx) as u8)) as u32)) & 0xff);
                ebx = (ebx & 0xffff_ff00) | (((((((((ebx) as u8)) as i32) & 0x1i32)) as u32)) & 0xff);
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                xmm1.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xh[1] = xmm1;
                if ((fa == 0)) { pc = 0x310369; continue; }
                pc = 0x310361; continue;
            }
            0x310361 => {
                xmm3 = Xmm::scalar(m.read32(0x475cd4));
                pc = 0x310369; continue;
            }
            0x310369 => {
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                xh[0] = Xmm::scalar(m.read32(0x475cd4));
                if (fa == fb) { pc = 0x31037d; continue; }
                pc = 0x310375; continue;
            }
            0x310375 => {
                xh[0] = Xmm::scalar(m.read32(0x475d50));
                pc = 0x31037d; continue;
            }
            0x31037d => {
                ecx = (((((m.read16(edi.wrapping_add(eax).wrapping_add(0x3074)) as i16)) as i32)) as u32);
                fa = ecx;
                fb = m.read32(eax.wrapping_add(0x264));
                if (fa != fb) { pc = 0x3108a4; continue; }
                pc = 0x310391; continue;
            }
            0x310391 => {
                ecx = ((m.read8(edi.wrapping_add(eax).wrapping_add(0x3077))) as u8 as u32);
                ecx = ecx.wrapping_sub(0);
                fa = ecx;
                if ((fa == 0)) { pc = 0x3103af; continue; }
                pc = 0x31039e; continue;
            }
            0x31039e => {
                ecx = ecx.wrapping_sub(1);
                fa = ecx;
                if ((fa != 0)) { pc = 0x3108a4; continue; }
                pc = 0x3103a5; continue;
            }
            0x3103a5 => {
                m.write32(esp.wrapping_add(0x18), 0x50507880);
                pc = 0x3103f4; continue;
            }
            0x3103af => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(edi.wrapping_add(eax).wrapping_add(0x3078))) as u32)) & 0xff);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x3103c4; continue; }
                pc = 0x3103ba; continue;
            }
            0x3103ba => {
                m.write32(esp.wrapping_add(0x18), 0x781e1e80);
                pc = 0x3103f4; continue;
            }
            0x3103c4 => {
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x3103d2; continue; }
                pc = 0x3103c8; continue;
            }
            0x3103c8 => {
                m.write32(esp.wrapping_add(0x18), 0x32326e80);
                pc = 0x3103f4; continue;
            }
            0x3103d2 => {
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x3103e0; continue; }
                pc = 0x3103d6; continue;
            }
            0x3103d6 => {
                m.write32(esp.wrapping_add(0x18), 0x6e6e3280);
                pc = 0x3103f4; continue;
            }
            0x3103e0 => {
                eax = (eax & 0xffff_ff00) | (((((((((eax) as u8)) as i32) & 0x8i32)) as u32)) & 0xff);
                cf = (((((((((eax) as u8))) as i32) != 0i32))) as i32);
                eax = (eax & 0xffff_ff00) | (((((((((eax) as u8)) as i32).wrapping_neg())) as u32)) & 0xff);
                eax = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                eax = (eax & 0xba231400u32);
                eax = eax.wrapping_add(0x784b1e80);
                m.write32(esp.wrapping_add(0x18), eax);
                pc = 0x3103f4; continue;
            }
            0x3103f4 => {
                fa = ((((((edx) as u8))) as u32) & 0xffu32);
                fb = ((((0xbi32)) as u32) & 0xffu32);
                if (fa < fb) { pc = 0x3106c8; continue; }
                pc = 0x3103fd; continue;
            }
            0x3103fd => {
                fa = ((((((edx) as u8))) as u32) & 0xffu32);
                fb = ((((0xei32)) as u32) & 0xffu32);
                if (fa > fb) { pc = 0x3106c8; continue; }
                pc = 0x310406; continue;
            }
            0x310406 => {
                fa = ((((((edx) as u8))) as u32) & 0xffu32);
                fb = ((((0xdi32)) as u32) & 0xffu32);
                m.write32(esp.wrapping_add(0x14), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                if (fa == fb) { pc = 0x310555; continue; }
                pc = 0x31041b; continue;
            }
            0x31041b => {
                fa = ((((((edx) as u8))) as u32) & 0xffu32);
                fb = ((((0xbi32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x310555; continue; }
                pc = 0x310424; continue;
            }
            0x310424 => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x10))) as f64));
                xmm2 = Xmm::scalar(m.read32(0x475dc4));
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x49f7bc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(0x5a5a5a80);
                push!(0x5a5a5a80);
                push!(0x5a5a5a80);
                m.write32(esp.wrapping_add(0x3c), (fpu.st(0) as f32).to_bits()); fpu.pop();
                push!(0x5a5a5a80);
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x24))) as f64));
                esi = m.read32(esp.wrapping_add(0x40));
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475dc4)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(esi);
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x40), (fpu.st(0) as f32).to_bits()); fpu.pop();
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x24))) as f64));
                ebx = m.read32(esp.wrapping_add(0x40));
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475c78)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(ebx);
                m.write32(esp.wrapping_add(0x54), (fpu.st(0) as f32).to_bits()); fpu.pop();
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x28))) as f64));
                edx = m.read32(esp.wrapping_add(0x54));
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475dc4)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(edx);
                push!(ecx);
                m.write32(esp, xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x70), (fpu.st(0) as f32).to_bits()); fpu.pop();
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x34))) as f64));
                ebp = m.read32(esp.wrapping_add(0x70));
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x49f7bc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(ebp);
                m.write32(esp.wrapping_add(0x4c), (fpu.st(0) as f32).to_bits()); fpu.pop();
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x38))) as f64));
                eax = m.read32(esp.wrapping_add(0x4c));
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475c78)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(eax);
                push!(ecx);
                m.write32(esp, xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x4c), (fpu.st(0) as f32).to_bits()); fpu.pop();
                ecx = m.read32(esp.wrapping_add(0x4c));
                push!(ecx);
                call!(0x308c00);
                pc = 0x3104cb; continue;
            }
            0x3104cb => {
                edx = m.read32(esp.wrapping_add(0x58));
                eax = m.read32(esp.wrapping_add(0x40));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x50)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475dc4))).to_bits();
                esp = esp.wrapping_add(0x30);
                push!(0x5a5a5a80);
                push!(ebp);
                push!(edx);
                push!(eax);
                push!(ecx);
                m.write32(esp, xmm0.lanes[0]);
                call!(0x308b10);
                pc = 0x3104f7; continue;
            }
            0x3104f7 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x50)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475dc4))).to_bits();
                esp = esp.wrapping_add(0x14);
                push!(0x5a5a5a80);
                push!(esi);
                push!(ebx);
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x24));
                m.write32(esp, xmm0.lanes[0]);
                push!(ecx);
                call!(0x308b10);
                pc = 0x31051f; continue;
            }
            0x31051f => {
                ecx = m.read32(0x7844a8);
                edx = (((((m.read16(edi.wrapping_add(ecx).wrapping_add(0x3072)) as i16)) as i32)) as u32);
                eax = (eax & 0xffff_0000) | ((((m.read16(edi.wrapping_add(ecx).wrapping_add(0x3070))) as u32)) & 0xffff);
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475c78))).to_bits();
                ecx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                xmm0 = Xmm::scalar(m.read32(0x475d2c));
                esp = esp.wrapping_add(0x14);
                pc = 0x31067c; continue;
            }
            0x310555 => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x10))) as f64));
                push!(0x5a5a5a80);
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475c78)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x475dc4))).to_bits();
                push!(0x5a5a5a80);
                push!(0x5a5a5a80);
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(0x475dc4)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(0x5a5a5a80);
                m.write32(esp.wrapping_add(0x60), (fpu.st(0) as f32).to_bits()); fpu.pop();
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x24))) as f64));
                ebp = m.read32(esp.wrapping_add(0x60));
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x49f7bc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(ebp);
                m.write32(esp.wrapping_add(0x3c), (fpu.st(0) as f32).to_bits()); fpu.pop();
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x24))) as f64));
                esi = m.read32(esp.wrapping_add(0x3c));
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x49f7bc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(esi);
                m.write32(esp.wrapping_add(0x48), (fpu.st(0) as f32).to_bits()); fpu.pop();
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x2c))) as f64));
                ebx = m.read32(esp.wrapping_add(0x48));
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475c78)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(ebx);
                m.write32(esp.wrapping_add(0x3c), (fpu.st(0) as f32).to_bits()); fpu.pop();
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x2c))) as f64));
                eax = m.read32(esp.wrapping_add(0x3c));
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(0x475dc4)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x30));
                m.write32(esp.wrapping_add(0x5c), (fpu.st(0) as f32).to_bits()); fpu.pop();
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x34))) as f64));
                ecx = m.read32(esp.wrapping_add(0x5c));
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475dc4)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(ecx);
                m.write32(esp.wrapping_add(0x50), (fpu.st(0) as f32).to_bits()); fpu.pop();
                edx = m.read32(esp.wrapping_add(0x50));
                push!(edx);
                push!(eax);
                push!(ecx);
                m.write32(esp, xmm1.lanes[0]);
                call!(0x308c00);
                pc = 0x3105f9; continue;
            }
            0x3105f9 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x40)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475dc4))).to_bits();
                edx = m.read32(esp.wrapping_add(0x6c));
                eax = m.read32(esp.wrapping_add(0x44));
                esp = esp.wrapping_add(0x30);
                push!(0x5a5a5a80);
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x34));
                m.write32(esp, xmm0.lanes[0]);
                push!(ecx);
                push!(edx);
                push!(eax);
                call!(0x308b10);
                pc = 0x310629; continue;
            }
            0x310629 => {
                ecx = m.read32(esp.wrapping_add(0x34));
                push!(0x5a5a5a80);
                push!(ebx);
                push!(esi);
                push!(ebp);
                push!(ecx);
                call!(0x308b10);
                pc = 0x31063b; continue;
            }
            0x31063b => {
                ecx = m.read32(0x7844a8);
                edx = (((((m.read16(edi.wrapping_add(ecx).wrapping_add(0x3070)) as i16)) as i32)) as u32);
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                ecx = (((((m.read16(edi.wrapping_add(ecx).wrapping_add(0x3072)) as i16)) as i32)) as u32);
                xmm1.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                eax = sse::cvttss2si(f32::from_bits(xmm1.lanes[0]));
                xmm1.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::scalar(m.read32(0x49f7a0));
                esp = esp.wrapping_add(0x28);
                ecx = sse::cvttss2si(f32::from_bits(xmm1.lanes[0]));
                pc = 0x31067c; continue;
            }
            0x31067c => {
                esi = m.read32(esp.wrapping_add(0x18));
                m.write32(esp.wrapping_add(0x40), xmm0.lanes[0]);
                edx = m.read32(esp.wrapping_add(0x40));
                push!(edx);
                push!(0x3dc00000);
                push!(0x3f69fbe8);
                push!(0xbd800000u32);
                ecx = ((((ecx) as u16)) as u16 as i16 as i32 as u32);
                push!(0x3f000000);
                esp = esp.wrapping_sub(0x8);
                edx = ((((eax) as u16)) as u16 as i16 as i32 as u32);
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                call!(0x30b7c0);
                pc = 0x3106c0; continue;
            }
            0x3106c0 => {
                esp = esp.wrapping_add(0x1c);
                pc = 0x31089f; continue;
            }
            0x3106c8 => {
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x3106e4; continue; }
                pc = 0x3106cc; continue;
            }
            0x3106cc => {
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(0x475d4c))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475dc4))).to_bits();
                m.write32(esp.wrapping_add(0x24), xh[1].lanes[0]);
                pc = 0x3106fa; continue;
            }
            0x3106e4 => {
                xh[1].lanes[0] = sse::subss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(0x475dc4))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d4c))).to_bits();
                m.write32(esp.wrapping_add(0x24), xh[1].lanes[0]);
                pc = 0x3106fa; continue;
            }
            0x3106fa => {
                xmm4.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                xh[2] = Xmm::scalar(m.read32(0x783f28));
                m.write32(esp.wrapping_add(0x1c), xmm0.lanes[0]);
                xmm0.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x1c)))).to_bits();
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x1c)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x34), xmm3.lanes[0]);
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(0x783f2c))).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm2 = xh[1];
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783f2c))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = xh[0];
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = Xmm::scalar(m.read32(0x475c78));
                m.write32(esp.wrapping_add(0x44), xmm0.lanes[0]);
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x58), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0x54), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x48), xmm3.lanes[0]);
                m.write32(esp.wrapping_add(0x38), xh[0].lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x3107b5; continue; }
                pc = 0x3107ab; continue;
            }
            0x3107ab => {
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x44), xmm1.lanes[0]);
                pc = 0x3107b5; continue;
            }
            0x3107b5 => {
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm3.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x3107c8; continue; }
                pc = 0x3107be; continue;
            }
            0x3107be => {
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x48), xmm2.lanes[0]);
                pc = 0x3107c8; continue;
            }
            0x3107c8 => {
                eax = m.read32(esp.wrapping_add(0x18));
                ecx = m.read32(esp.wrapping_add(0x44));
                edx = m.read32(esp.wrapping_add(0x48));
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x58));
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x60));
                push!(edx);
                push!(eax);
                push!(ecx);
                call!(0x1a2e0);
                pc = 0x3107e6; continue;
            }
            0x3107e6 => {
                edx = m.read32(0x7844a8);
                eax = (eax & 0xffff_ff00) | ((((m.read8(edi.wrapping_add(edx).wrapping_add(0x3076))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x14);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                push!(0x5a5a5a80);
                if ((fa & fb) == 0) { pc = 0x31084c; continue; }
                pc = 0x3107ff; continue;
            }
            0x3107ff => {
                esi = m.read32(esp.wrapping_add(0x3c));
                eax = m.read32(esp.wrapping_add(0x28));
                ebx = m.read32(esp.wrapping_add(0x20));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x28)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d4c))).to_bits();
                push!(esi);
                push!(eax);
                push!(ebx);
                push!(ecx);
                m.write32(esp, xmm0.lanes[0]);
                call!(0x308b10);
                pc = 0x310827; continue;
            }
            0x310827 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x48)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d4c))).to_bits();
                esp = esp.wrapping_add(0x14);
                push!(0x5a5a5a80);
                push!(esi);
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x40));
                m.write32(esp, xmm0.lanes[0]);
                push!(ebx);
                push!(ecx);
                pc = 0x310897; continue;
            }
            0x31084c => {
                edx = m.read32(esp.wrapping_add(0x20));
                esi = m.read32(esp.wrapping_add(0x38));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x20)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d4c))).to_bits();
                ebx = m.read32(esp.wrapping_add(0x28));
                push!(edx);
                push!(esi);
                push!(ecx);
                m.write32(esp, xmm0.lanes[0]);
                push!(ebx);
                call!(0x308b10);
                pc = 0x310874; continue;
            }
            0x310874 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x4c)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d4c))).to_bits();
                eax = m.read32(esp.wrapping_add(0x4c));
                esp = esp.wrapping_add(0x14);
                push!(0x5a5a5a80);
                push!(ecx);
                m.write32(esp, xmm0.lanes[0]);
                push!(esi);
                push!(eax);
                push!(ebx);
                pc = 0x310897; continue;
            }
            0x310897 => {
                call!(0x308b10);
                pc = 0x31089c; continue;
            }
            0x31089c => {
                esp = esp.wrapping_add(0x14);
                pc = 0x31089f; continue;
            }
            0x31089f => {
                eax = m.read32(0x7844a8);
                pc = 0x3108a4; continue;
            }
            0x3108a4 => {
                ecx = m.read32(esp.wrapping_add(0x4c));
                edx = ((m.read16(eax.wrapping_add(0x306e))) as u16 as u32);
                ecx = ecx.wrapping_add(1);
                edi = edi.wrapping_add(0xc);
                fa = ecx;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x4c), ecx);
                if (fas < fbs) { pc = 0x310330; continue; }
                pc = 0x3108bf; continue;
            }
            0x3108bf => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x3108c2; continue;
            }
            0x3108c2 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x4c);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x003108D0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3108d0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3108d0;
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
        { crate::pose::cover(0x3108d0, previous, pc); previous = pc; }
        match pc {
            0x3108d0 => {
                push!(ebp);
                ebp = esp;
                esp = (esp & 0xfffffff8u32);
                esp = esp.wrapping_sub(0x58);
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f7c));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                push!(ebx);
                push!(ebp);
                push!(esi);
                push!(edi);
                if (fa == fb) { pc = 0x310a17; continue; }
                pc = 0x3108f0; continue;
            }
            0x3108f0 => {
                ecx = m.read32(eax.wrapping_add(0x25c));
                xmm0.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x2f84)));
                xmm2 = Xmm::scalar(m.read32(eax.wrapping_add(0x2f88)));
                ebx = m.read32(eax.wrapping_add(0x2f7c));
                edx = 0;
                ecx = (ecx & 0x410004);
                fa = ecx;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                edx = (edx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783dd4))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783f20))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                edx = edx.wrapping_add(0x2);
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ebp = edx;
                m.write32(esp.wrapping_add(0x14), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x18), xmm2.lanes[0]);
                if (fa == fb) { pc = 0x310a17; continue; }
                pc = 0x31096b; continue;
            }
            0x31096b => {
                pc = 0x310980; continue;
            }
            0x31096d => {
                eax = m.read32(0x7844a8);
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x18)));
                edi = edi;
                pc = 0x310980; continue;
            }
            0x310980 => {
                ecx = 0x12;
                esi = ebx;
                edi = esp.wrapping_add(0x20);
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), m.read32(esi.wrapping_add(i.wrapping_mul(step)))); } esi = esi.wrapping_add(ecx.wrapping_mul(step)); edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep movsd, DF from entry.eflags
                ecx = 0;
                xmm0 = Xmm::scalar(m.read32(ebx));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::default();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x3109a5; continue; }
                pc = 0x31099d; continue;
            }
            0x31099d => {
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475c78))).to_bits();
                pc = 0x3109a5; continue;
            }
            0x3109a5 => {
                ecx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                m.write32(esp.wrapping_add(0x1c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ebx.wrapping_add(0x4)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x3109c9; continue; }
                pc = 0x3109c1; continue;
            }
            0x3109c1 => {
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475c78))).to_bits();
                pc = 0x3109c9; continue;
            }
            0x3109c9 => {
                xmm1.lanes[0] = (((m.read32(eax.wrapping_add(0x264))) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(ebx.wrapping_add(0x8)))).to_bits();
                push!(0x1);
                push!(0x3f800000);
                m.write32(esp.wrapping_add(0x30), xmm1.lanes[0]);
                eax = m.read32(ebx.wrapping_add(0xc));
                ecx = m.read32(ebx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x2c));
                eax = m.read32(esp.wrapping_add(0x24));
                edx = esp.wrapping_add(0x28);
                push!(edx);
                push!(ebp);
                push!(ecx);
                push!(ecx);
                edx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                push!(eax);
                call!(0x30de40);
                pc = 0x310a09; continue;
            }
            0x310a09 => {
                ebx = m.read32(ebx.wrapping_add(0x40));
                esp = esp.wrapping_add(0x1c);
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31096d; continue; }
                pc = 0x310a17; continue;
            }
            0x310a17 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = ebp;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}
}
pub use unformatted::*;
