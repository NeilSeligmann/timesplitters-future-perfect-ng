use super::*; // portreg: replaces=tsfp-ports/mapmaker_ui_lane
#[rustfmt::skip] mod unformatted { use super::*;
/// 0x003236C0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3236c0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3236c0;
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
        { crate::pose::cover(0x3236c0, previous, pc); previous = pc; }
        match pc {
            0x3236c0 => {
                esp = esp.wrapping_sub(0x28);
                fpu.push((f32::from_bits(m.read32(0x7ba038)) as f64));
                push!(ebx);
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4760cc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(ebp);
                push!(esi);
                esi = eax;
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x7ba180)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                eax = m.read32(esi.wrapping_add(0x2c));
                push!(edi);
                push!(0);
                m.write32(esp.wrapping_add(0x18), (fpu.st(0) as f32).to_bits()); fpu.pop();
                fpu.push((f32::from_bits(m.read32(0x7ba188)) as f64));
                push!(esi);
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x475d18)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(eax.wrapping_add(0xf0))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x18), (fpu.st(0) as f32).to_bits()); fpu.pop();
                call!(0x70d30);
                pc = 0x323700; continue;
            }
            0x323700 => {
                push!(0x1);
                push!(esi);
                ebp = eax;
                call!(0x70d30);
                pc = 0x32370a; continue;
            }
            0x32370a => {
                fpu.push((f32::from_bits(m.read32(esi.wrapping_add(0xf4))) as f64));
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x475cc0)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x30), eax);
                call!(0x3c848c);
                pc = 0x32371f; continue;
            }
            0x32371f => {
                fpu.push((f32::from_bits(m.read32(esi.wrapping_add(0xf4))) as f64));
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x47cff4)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                ebx = eax;
                call!(0x3c848c);
                pc = 0x323732; continue;
            }
            0x323732 => {
                ecx = esp.wrapping_add(0x38);
                push!(ecx);
                edx = esp.wrapping_add(0x40);
                edi = eax;
                push!(edx);
                eax = esp.wrapping_add(0x48);
                push!(eax);
                ecx = esp.wrapping_add(0x50);
                push!(ecx);
                call!(0x19fc0);
                pc = 0x32374d; continue;
            }
            0x32374d => {
                edx = m.read32(0x4e8cf4);
                eax = m.read32(0x4e8cf0);
                push!(edx);
                push!(eax);
                push!(0);
                push!(0);
                call!(0x1a010);
                pc = 0x323763; continue;
            }
            0x323763 => {
                push!(0xf);
                call!(0x3fa10);
                pc = 0x32376a; continue;
            }
            0x32376a => {
                push!(0xb93);
                call!(0x3d7b0);
                pc = 0x323774; continue;
            }
            0x323774 => {
                ecx = m.read32(esp.wrapping_add(0x58));
                edx = m.read32(esp.wrapping_add(0x48));
                push!(ecx);
                esi = eax;
                eax = m.read32(esp.wrapping_add(0x50));
                push!(ebp);
                push!(esi);
                push!(edx);
                push!(eax);
                call!(0x41e30);
                pc = 0x32378c; continue;
            }
            0x32378c => {
                esp = esp.wrapping_add(0x4c);
                push!(esi);
                call!(0x40620);
                pc = 0x323795; continue;
            }
            0x323795 => {
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(esp.wrapping_add(0x14))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                ebp = m.read32(esp.wrapping_add(0x40));
                eax = m.read32(ebp);
                esp = esp.wrapping_add(0x4);
                m.write32(esp.wrapping_add(0x10), (fpu.st(0) as f32).to_bits()); fpu.pop();
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x3237d0; continue; }
                pc = 0x3237ac; continue;
            }
            0x3237ac => {
                fa = eax;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x3237d0; continue; }
                pc = 0x3237b1; continue;
            }
            0x3237b1 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fa == fb) { pc = 0x3237c5; continue; }
                pc = 0x3237b5; continue;
            }
            0x3237b5 => {
                if (fas <= fbs) { pc = 0x3237d0; continue; }
                pc = 0x3237b7; continue;
            }
            0x3237b7 => {
                fa = eax;
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x3237d0; continue; }
                pc = 0x3237bc; continue;
            }
            0x3237bc => {
                eax = ebp;
                call!(0x31e950);
                pc = 0x3237c3; continue;
            }
            0x3237c3 => {
                pc = 0x3237cc; continue;
            }
            0x3237c5 => {
                eax = ebp;
                call!(0x323560);
                pc = 0x3237cc; continue;
            }
            0x3237cc => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x32380c; continue; }
                pc = 0x3237d0; continue;
            }
            0x3237d0 => {
                push!(0xb94);
                call!(0x3d7b0);
                pc = 0x3237da; continue;
            }
            0x3237da => {
                ecx = m.read32(esp.wrapping_add(0x14));
                edx = m.read32(esp.wrapping_add(0x18));
                edi = (edi & 0xff);
                edi = (edi | 0x64646400);
                push!(edi);
                ebx = (ebx & 0xff);
                ebx = (ebx | 0xa7f0a00);
                push!(ebx);
                push!(eax);
                push!(ecx);
                push!(edx);
                call!(0x41e30);
                pc = 0x323804; continue;
            }
            0x323804 => {
                esp = esp.wrapping_add(0x18);
                pc = 0x323b20; continue;
            }
            0x32380c => {
                ecx = m.read32(ebp);
                esi = ebx;
                esi = (esi & 0xff);
                eax = esi;
                eax = (eax | 0x64646400);
                m.write32(esp.wrapping_add(0x1c), eax);
                eax = 0;
                fa = ecx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x3c), 0);
                if (fas <= fbs) { pc = 0x32385b; continue; }
                pc = 0x323831; continue;
            }
            0x323831 => {
                fa = ecx;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x32385b; continue; }
                pc = 0x323836; continue;
            }
            0x323836 => {
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fa == fb) { pc = 0x32384e; continue; }
                pc = 0x32383a; continue;
            }
            0x32383a => {
                if (fas <= fbs) { pc = 0x323855; continue; }
                pc = 0x32383c; continue;
            }
            0x32383c => {
                fa = ecx;
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x323855; continue; }
                pc = 0x323841; continue;
            }
            0x323841 => {
                eax = ebp;
                call!(0x31e950);
                pc = 0x323848; continue;
            }
            0x323848 => {
                m.write32(esp.wrapping_add(0x18), eax);
                pc = 0x323863; continue;
            }
            0x32384e => {
                eax = ebp;
                call!(0x323560);
                pc = 0x323855; continue;
            }
            0x323855 => {
                m.write32(esp.wrapping_add(0x18), eax);
                pc = 0x323863; continue;
            }
            0x32385b => {
                m.write32(esp.wrapping_add(0x18), 0);
                pc = 0x323863; continue;
            }
            0x323863 => {
                xmm0 = Xmm::scalar(m.read32(0x7ba038));
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x475d08))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x7ba180))).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x14)))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d20))).to_bits();
                push!(0);
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                push!(0x40);
                m.write32(esp.wrapping_add(0x2c), xmm1.lanes[0]);
                call!(0x3dfa0);
                pc = 0x32389f; continue;
            }
            0x32389f => {
                push!(0xb95);
                m.write32(esp.wrapping_add(0x2c), eax);
                call!(0x3d7b0);
                pc = 0x3238ad; continue;
            }
            0x3238ad => {
                ecx = m.read32(esp.wrapping_add(0x20));
                edi = (edi & 0xff);
                edi = (edi | 0x1e0a0a00);
                push!(edi);
                ebx = eax;
                eax = m.read32(esp.wrapping_add(0x20));
                esi = (esi | 0x7f0a0a00);
                push!(esi);
                push!(ebx);
                push!(eax);
                push!(ecx);
                call!(0x41e30);
                pc = 0x3238d3; continue;
            }
            0x3238d3 => {
                push!(ebx);
                call!(0x40620);
                pc = 0x3238d9; continue;
            }
            0x3238d9 => {
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(esp.wrapping_add(0x34))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(0x11);
                m.write32(esp.wrapping_add(0x38), (fpu.st(0) as f32).to_bits()); fpu.pop();
                call!(0x3fa10);
                pc = 0x3238e8; continue;
            }
            0x3238e8 => {
                ebx = m.read32(esp.wrapping_add(0x40));
                esp = esp.wrapping_add(0x28);
                esi = 0;
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((0x40i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x3238fd; continue; }
                pc = 0x3238f6; continue;
            }
            0x3238f6 => {
                push!(0xb9c);
                pc = 0x32393c; continue;
            }
            0x3238fd => {
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (((((((((((((fa)) as u8)) as i32).wrapping_sub((((((fb)) as u8)) as i32)))) as u8)) as i32).wrapping_shr((0x7 & 31))) == 0i32)) { pc = 0x323926; continue; }
                pc = 0x323901; continue;
            }
            0x323901 => {
                eax = 0x52e5b4;
                pc = 0x323906; continue;
            }
            0x323906 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x323911; continue; }
                pc = 0x32390a; continue;
            }
            0x32390a => {
                edx = m.read32(eax);
                fa = edx;
                fb = m.read32(ebp.wrapping_add(0x4));
                if (fa == fb) { pc = 0x323920; continue; }
                pc = 0x323911; continue;
            }
            0x323911 => {
                eax = eax.wrapping_add(0xc);
                fa = eax;
                fb = ((((0x52e5d8i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x323906; continue; }
                pc = 0x32391b; continue;
            }
            0x32391b => {
                eax = 0;
                push!(eax);
                pc = 0x32393c; continue;
            }
            0x323920 => {
                eax = m.read32(eax.wrapping_add(0x8));
                push!(eax);
                pc = 0x32393c; continue;
            }
            0x323926 => {
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x323932; continue; }
                pc = 0x32392b; continue;
            }
            0x32392b => {
                push!(0xba0);
                pc = 0x32393c; continue;
            }
            0x323932 => {
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x323946; continue; }
                pc = 0x323937; continue;
            }
            0x323937 => {
                push!(0xba1);
                pc = 0x32393c; continue;
            }
            0x32393c => {
                call!(0x3d7b0);
                pc = 0x323941; continue;
            }
            0x323941 => {
                esp = esp.wrapping_add(0x4);
                esi = eax;
                pc = 0x323946; continue;
            }
            0x323946 => {
                eax = ebx;
                eax = (eax & 0x1);
                fa = eax;
                if ((fa == 0)) { pc = 0x323959; continue; }
                pc = 0x32394d; continue;
            }
            0x32394d => {
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x323959; continue; }
                pc = 0x323952; continue;
            }
            0x323952 => {
                push!(0xb96);
                pc = 0x32396e; continue;
            }
            0x323959 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x323964; continue; }
                pc = 0x32395d; continue;
            }
            0x32395d => {
                push!(0xb97);
                pc = 0x32396e; continue;
            }
            0x323964 => {
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x323978; continue; }
                pc = 0x323969; continue;
            }
            0x323969 => {
                push!(0xb98);
                pc = 0x32396e; continue;
            }
            0x32396e => {
                call!(0x3d7b0);
                pc = 0x323973; continue;
            }
            0x323973 => {
                esp = esp.wrapping_add(0x4);
                esi = eax;
                pc = 0x323978; continue;
            }
            0x323978 => {
                eax = ebx;
                eax = (eax & 0x4);
                fa = eax;
                if ((fa == 0)) { pc = 0x32399a; continue; }
                pc = 0x32397f; continue;
            }
            0x32397f => {
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((0x8i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x32399a; continue; }
                pc = 0x323984; continue;
            }
            0x323984 => {
                push!(0xb99);
                call!(0x3d7b0);
                pc = 0x32398e; continue;
            }
            0x32398e => {
                esp = esp.wrapping_add(0x4);
                m.write32(esp.wrapping_add(0x3c), eax);
                pc = 0x323ab5; continue;
            }
            0x32399a => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3239b4; continue; }
                pc = 0x32399e; continue;
            }
            0x32399e => {
                push!(0xb9a);
                call!(0x3d7b0);
                pc = 0x3239a8; continue;
            }
            0x3239a8 => {
                esp = esp.wrapping_add(0x4);
                m.write32(esp.wrapping_add(0x3c), eax);
                pc = 0x323ab5; continue;
            }
            0x3239b4 => {
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((0x8i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x3239cf; continue; }
                pc = 0x3239b9; continue;
            }
            0x3239b9 => {
                push!(0xb9b);
                call!(0x3d7b0);
                pc = 0x3239c3; continue;
            }
            0x3239c3 => {
                esp = esp.wrapping_add(0x4);
                m.write32(esp.wrapping_add(0x3c), eax);
                pc = 0x323ab5; continue;
            }
            0x3239cf => {
                fa = (((((((ebx) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x3239ea; continue; }
                pc = 0x3239d4; continue;
            }
            0x3239d4 => {
                push!(0xba7);
                call!(0x3d7b0);
                pc = 0x3239de; continue;
            }
            0x3239de => {
                esp = esp.wrapping_add(0x4);
                m.write32(esp.wrapping_add(0x3c), eax);
                pc = 0x323ab5; continue;
            }
            0x3239ea => {
                eax = ebx;
                eax = (eax & 0x200);
                fa = eax;
                if ((fa == 0)) { pc = 0x323a0e; continue; }
                pc = 0x3239f3; continue;
            }
            0x3239f3 => {
                fa = (((((((ebx) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x323a0e; continue; }
                pc = 0x3239f8; continue;
            }
            0x3239f8 => {
                push!(0xba8);
                call!(0x3d7b0);
                pc = 0x323a02; continue;
            }
            0x323a02 => {
                esp = esp.wrapping_add(0x4);
                m.write32(esp.wrapping_add(0x3c), eax);
                pc = 0x323ab5; continue;
            }
            0x323a0e => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x323a28; continue; }
                pc = 0x323a12; continue;
            }
            0x323a12 => {
                push!(0xba9);
                call!(0x3d7b0);
                pc = 0x323a1c; continue;
            }
            0x323a1c => {
                esp = esp.wrapping_add(0x4);
                m.write32(esp.wrapping_add(0x3c), eax);
                pc = 0x323ab5; continue;
            }
            0x323a28 => {
                fa = (((((((ebx) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x323a40; continue; }
                pc = 0x323a2d; continue;
            }
            0x323a2d => {
                push!(0xbaa);
                call!(0x3d7b0);
                pc = 0x323a37; continue;
            }
            0x323a37 => {
                esp = esp.wrapping_add(0x4);
                m.write32(esp.wrapping_add(0x3c), eax);
                pc = 0x323ab5; continue;
            }
            0x323a40 => {
                fa = (((((((ebx) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x8i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x323a58; continue; }
                pc = 0x323a45; continue;
            }
            0x323a45 => {
                push!(0xbab);
                call!(0x3d7b0);
                pc = 0x323a4f; continue;
            }
            0x323a4f => {
                esp = esp.wrapping_add(0x4);
                m.write32(esp.wrapping_add(0x3c), eax);
                pc = 0x323ab5; continue;
            }
            0x323a58 => {
                fa = (((((((ebx) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x323ab5; continue; }
                pc = 0x323a5d; continue;
            }
            0x323a5d => {
                ecx = (((((m.read16(ebp.wrapping_add(0xc)) as i16)) as i32)) as u32);
                eax = m.read32(0x7844a8);
                ecx = ((ecx as i32).wrapping_mul((0x8c) as i32)) as u32;
                edx = m.read32(ecx.wrapping_add(eax).wrapping_add(0xdd90));
                edx = ((edx as i32).wrapping_mul((0xb4) as i32)) as u32;
                eax = edx.wrapping_add(eax).wrapping_add(0x81a8);
                eax = m.read32(eax.wrapping_add(0x8));
                ecx = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x52e5d8));
                push!(ecx);
                call!(0x3d7b0);
                pc = 0x323a90; continue;
            }
            0x323a90 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0xba2);
                call!(0x3d7b0);
                pc = 0x323a9e; continue;
            }
            0x323a9e => {
                edi = m.read32(esp.wrapping_add(0x28));
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x3f);
                push!(edi);
                call!(0x3ca0b5);
                pc = 0x323aae; continue;
            }
            0x323aae => {
                esp = esp.wrapping_add(0x10);
                m.write32(esp.wrapping_add(0x3c), edi);
                pc = 0x323ab5; continue;
            }
            0x323ab5 => {
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                edi = m.read32(esp.wrapping_add(0x24));
                if (fa == fb) { pc = 0x323aec; continue; }
                pc = 0x323abd; continue;
            }
            0x323abd => {
                push!(edi);
                push!(esi);
                call!(0x40f00);
                pc = 0x323ac4; continue;
            }
            0x323ac4 => {
                edx = m.read32(esp.wrapping_add(0x24));
                ecx = m.read32(esp.wrapping_add(0x1c));
                push!(edx);
                esi = eax;
                eax = m.read32(esp.wrapping_add(0x1c));
                push!(esi);
                push!(eax);
                push!(ecx);
                call!(0x41dc0);
                pc = 0x323adb; continue;
            }
            0x323adb => {
                push!(esi);
                call!(0x40620);
                pc = 0x323ae1; continue;
            }
            0x323ae1 => {
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(esp.wrapping_add(0x2c))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                esp = esp.wrapping_add(0x1c);
                m.write32(esp.wrapping_add(0x10), (fpu.st(0) as f32).to_bits()); fpu.pop();
                pc = 0x323aec; continue;
            }
            0x323aec => {
                eax = m.read32(esp.wrapping_add(0x3c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x323b13; continue; }
                pc = 0x323af4; continue;
            }
            0x323af4 => {
                push!(edi);
                push!(eax);
                call!(0x40f00);
                pc = 0x323afb; continue;
            }
            0x323afb => {
                edx = m.read32(esp.wrapping_add(0x24));
                ecx = m.read32(esp.wrapping_add(0x1c));
                push!(edx);
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x20));
                push!(eax);
                push!(ecx);
                call!(0x41dc0);
                pc = 0x323b10; continue;
            }
            0x323b10 => {
                esp = esp.wrapping_add(0x18);
                pc = 0x323b13; continue;
            }
            0x323b13 => {
                edx = m.read32(esp.wrapping_add(0x20));
                push!(edx);
                call!(0x3e1e0);
                pc = 0x323b1d; continue;
            }
            0x323b1d => {
                esp = esp.wrapping_add(0x4);
                pc = 0x323b20; continue;
            }
            0x323b20 => {
                eax = m.read32(esp.wrapping_add(0x28));
                ecx = m.read32(esp.wrapping_add(0x2c));
                edx = m.read32(esp.wrapping_add(0x30));
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x38));
                push!(ecx);
                push!(edx);
                push!(eax);
                call!(0x1a010);
                pc = 0x323b39; continue;
            }
            0x323b39 => {
                esp = esp.wrapping_add(0x10);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x28);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}
/// 0x00323C30: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f323c30<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x323c30;
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
        { crate::pose::cover(0x323c30, previous, pc); previous = pc; }
        match pc {
            0x323c30 => {
                esp = esp.wrapping_sub(0x70);
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x475cac)) as f64);
                push!(ebx);
                ebx = eax;
                eax = (((eax & 0xffff00ffu32)) | ((((((if ((((fca == fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x40u32 } else { 0x0u32 }) | (if ((((fca != fca) || (fcb != fcb)))) { 0x4u32 } else { 0x0u32 })) | (if ((((fca < fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x1u32 } else { 0x0u32 })) | 0x2u32)).wrapping_shl((0x8 & 31))));
                fa = (((((((eax) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x44i32)) as u32) & 0xffu32);
                push!(esi);
                push!(edi);
                esi = ecx;
                if (((((fa) & (fb))) as u8).count_ones() % 2 == 0) { pc = 0x323c4f; continue; }
                pc = 0x323c47; continue;
            }
            0x323c47 => {
                xmm0 = Xmm::scalar(m.read32(0x47d01c));
                pc = 0x323c4f; continue;
            }
            0x323c4f => {
                eax = 0;
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x8c)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d18))).to_bits();
                ecx = 0x10;
                edi = esp.wrapping_add(0x3c);
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), eax); } edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep stosd, DF from entry.eflags
                ecx = 0;
                eax = m.read32(esp.wrapping_add(0x94));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0x8c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x4788fc));
                if (fa != fb) { pc = 0x323c97; continue; }
                pc = 0x323c8f; continue;
            }
            0x323c8f => {
                xmm0 = Xmm::scalar(m.read32(0x47862c));
                pc = 0x323c97; continue;
            }
            0x323c97 => {
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x475d10))).to_bits();
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0xc)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                push!(0x3);
                m.write32(esp.wrapping_add(0x1c), xmm0.lanes[0]);
                call!(0x3fa10);
                pc = 0x323cbf; continue;
            }
            0x323cbf => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x1c)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x4760cc))).to_bits();
                esp = esp.wrapping_add(0x4);
                push!(0x4);
                push!(ecx);
                m.write32(esp, xmm0.lanes[0]);
                push!(ebx);
                call!(0x40970);
                pc = 0x323cde; continue;
            }
            0x323cde => {
                eax = m.read32(esp.wrapping_add(0x24));
                push!(eax);
                push!(ebx);
                call!(0x40f00);
                pc = 0x323ce9; continue;
            }
            0x323ce9 => {
                esp = esp.wrapping_add(0x14);
                edx = esp.wrapping_add(0x3c);
                pc = 0x323cf0; continue;
            }
            0x323cf0 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax)) as u32)) & 0xff);
                eax = eax.wrapping_add(1);
                m.write8(edx, (((((ecx) as u8)) as u32)) as u8);
                edx = edx.wrapping_add(1);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x323cf0; continue; }
                pc = 0x323cfa; continue;
            }
            0x323cfa => {
                push!(0);
                ecx = esp.wrapping_add(0x40);
                push!(ecx);
                edx = esp.wrapping_add(0x28);
                push!(edx);
                eax = esp.wrapping_add(0x34);
                push!(eax);
                call!(0x3fcf0);
                pc = 0x323d10; continue;
            }
            0x323d10 => {
                xmm0 = Xmm::scalar(m.read32(0x7ba188));
                eax = m.read32(0x763080);
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d00))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x30)))).to_bits();
                esp = esp.wrapping_add(0x10);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0x18), xmm0.lanes[0]);
                if (fa == fb) { pc = 0x324011; continue; }
                pc = 0x323d3c; continue;
            }
            0x323d3c => {
                ecx = m.read32(esi.wrapping_add(0xf4));
                push!(ebp);
                push!(ecx);
                push!(0x6464647f);
                call!(0x154fe0);
                pc = 0x323d4e; continue;
            }
            0x323d4e => {
                edi = m.read32(esp.wrapping_add(0x8c));
                edx = m.read32(esi.wrapping_add(0xf4));
                edi = ((edi as i32).wrapping_mul((0x34) as i32)) as u32;
                m.write32(esp.wrapping_add(0x30), eax);
                eax = m.read32(edi.wrapping_add(0x52e6d4));
                push!(edx);
                push!(eax);
                call!(0x154fe0);
                pc = 0x323d6f; continue;
            }
            0x323d6f => {
                ecx = m.read32(esi.wrapping_add(0xf4));
                edx = m.read32(edi.wrapping_add(0x52e6d8));
                push!(ecx);
                push!(edx);
                ebx = eax;
                call!(0x154fe0);
                pc = 0x323d84; continue;
            }
            0x323d84 => {
                edi = eax;
                eax = m.read32(esi.wrapping_add(0xf4));
                push!(eax);
                push!(0x7f7f7f7f);
                call!(0x154fe0);
                pc = 0x323d97; continue;
            }
            0x323d97 => {
                xmm0 = Xmm::default();
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0xb0)));
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0xb4)));
                m.write32(esp.wrapping_add(0x40), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x30)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                ebp = eax;
                eax = m.read32(esp.wrapping_add(0xb8));
                m.write32(esp.wrapping_add(0x58), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x3c)));
                esp = esp.wrapping_add(0x20);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x18), 0);
                m.write32(esp.wrapping_add(0x30), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0x34), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x3c), xmm0.lanes[0]);
                if (fa == fb) { pc = 0x323e0c; continue; }
                pc = 0x323df6; continue;
            }
            0x323df6 => {
                xmm0 = Xmm::scalar(m.read32(0x7ba038));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d40))).to_bits();
                m.write32(esp.wrapping_add(0x20), xmm0.lanes[0]);
                pc = 0x323e0c; continue;
            }
            0x323e0c => {
                eax = m.read32(0x763068);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x323e43; continue; }
                pc = 0x323e15; continue;
            }
            0x323e15 => {
                ecx = esp.wrapping_add(0x30);
                push!(ecx);
                call!(0x711f0);
                pc = 0x323e1f; continue;
            }
            0x323e1f => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x323e43; continue; }
                pc = 0x323e26; continue;
            }
            0x323e26 => {
                eax = m.read32(esp.wrapping_add(0xa4));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x323e43; continue; }
                pc = 0x323e31; continue;
            }
            0x323e31 => {
                xmm0 = Xmm::scalar(m.read32(0x475d40));
                m.write32(esp.wrapping_add(0x18), 0x1);
                pc = 0x323e4b; continue;
            }
            0x323e43 => {
                xmm0 = Xmm::scalar(m.read32(0x475d04));
                pc = 0x323e4b; continue;
            }
            0x323e4b => {
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                edx = m.read32(esp.wrapping_add(0x10));
                push!(edx);
                push!(edi);
                call!(0x154e90);
                pc = 0x323e5c; continue;
            }
            0x323e5c => {
                xmm0 = Xmm::scalar(m.read32(0x476040));
                edi = eax;
                eax = m.read32(esp.wrapping_add(0x20));
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x323e79; continue; }
                pc = 0x323e71; continue;
            }
            0x323e71 => {
                xmm0 = Xmm::scalar(m.read32(0x475d40));
                pc = 0x323e79; continue;
            }
            0x323e79 => {
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                eax = m.read32(esp.wrapping_add(0x10));
                push!(eax);
                push!(ebx);
                call!(0x154e90);
                pc = 0x323e8a; continue;
            }
            0x323e8a => {
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x2c));
                push!(eax);
                ecx = esp.wrapping_add(0x40);
                push!(edi);
                push!(ecx);
                call!(0x30d120);
                pc = 0x323e9b; continue;
            }
            0x323e9b => {
                push!(ebp);
                push!(0x40400000);
                edx = esp.wrapping_add(0x50);
                push!(edi);
                push!(edx);
                call!(0x30d300);
                pc = 0x323eac; continue;
            }
            0x323eac => {
                ebx = m.read32(esp.wrapping_add(0xc4));
                edi = m.read32(esp.wrapping_add(0xb4));
                esp = esp.wrapping_add(0x28);
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x323f0b; continue; }
                pc = 0x323ec1; continue;
            }
            0x323ec1 => {
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x323f31; continue; }
                pc = 0x323ec5; continue;
            }
            0x323ec5 => {
                push!(edi);
                call!(0x323690);
                pc = 0x323ecb; continue;
            }
            0x323ecb => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x323f0b; continue; }
                pc = 0x323ed2; continue;
            }
            0x323ed2 => {
                eax = m.read32(esi.wrapping_add(0xf4));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x34)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475db4))).to_bits();
                push!(eax);
                esp = esp.wrapping_sub(0x8);
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x44)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x47d1e4))).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                call!(0x31eb40);
                pc = 0x323f08; continue;
            }
            0x323f08 => {
                esp = esp.wrapping_add(0xc);
                pc = 0x323f0b; continue;
            }
            0x323f0b => {
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x323f31; continue; }
                pc = 0x323f0f; continue;
            }
            0x323f0f => {
                ebp = m.read32(esp.wrapping_add(0x84));
                fa = ebp;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x323f38; continue; }
                pc = 0x323f1b; continue;
            }
            0x323f1b => {
                fa = ebp;
                fb = ((((0x6i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x323f38; continue; }
                pc = 0x323f20; continue;
            }
            0x323f20 => {
                push!(ebp);
                eax = esp.wrapping_add(0x34);
                ecx = esi;
                call!(0x31eab0);
                pc = 0x323f2c; continue;
            }
            0x323f2c => {
                esp = esp.wrapping_add(0x4);
                pc = 0x323f38; continue;
            }
            0x323f31 => {
                ebp = m.read32(esp.wrapping_add(0x84));
                pc = 0x323f38; continue;
            }
            0x323f38 => {
                ecx = m.read32(esp.wrapping_add(0x28));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x1c)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x24)))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475cd4))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x94)))).to_bits();
                push!(ecx);
                edx = esp.wrapping_add(0x44);
                push!(edx);
                esp = esp.wrapping_sub(0x8);
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0xa0)))).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                call!(0x41dc0);
                pc = 0x323f81; continue;
            }
            0x323f81 => {
                push!(0x3f800000);
                push!(0x3f800000);
                push!(0x3f800000);
                call!(0x3f9c0);
                pc = 0x323f95; continue;
            }
            0x323f95 => {
                eax = m.read32(esp.wrapping_add(0x34));
                esp = esp.wrapping_add(0x1c);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x324010; continue; }
                pc = 0x323fa0; continue;
            }
            0x323fa0 => {
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x323fb3; continue; }
                pc = 0x323fa4; continue;
            }
            0x323fa4 => {
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x323fb3; continue; }
                pc = 0x323fa8; continue;
            }
            0x323fa8 => {
                push!(edi);
                eax = esi;
                call!(0x3236c0);
                pc = 0x323fb0; continue;
            }
            0x323fb0 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x323fb3; continue;
            }
            0x323fb3 => {
                call!(0x31ec90);
                pc = 0x323fb8; continue;
            }
            0x323fb8 => {
                fa = ((((m.read8(esi.wrapping_add(0xc4)))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x323ffe; continue; }
                pc = 0x323fc1; continue;
            }
            0x323fc1 => {
                eax = m.read32(esp.wrapping_add(0xa4));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x323ffe; continue; }
                pc = 0x323fcc; continue;
            }
            0x323fcc => {
                ecx = m.read32(0x7844a8);
                ecx = (((((m.read8(ecx.wrapping_add(0x11854)) as i8)) as i32)) as u32);
                ecx = ecx.wrapping_sub(1);
                fa = ecx;
                if ((fa != 0)) { pc = 0x323fee; continue; }
                pc = 0x323fdc; continue;
            }
            0x323fdc => {
                edx = m.read32(esp.wrapping_add(0xa0));
                push!(edx);
                call!(0x314f80);
                pc = 0x323fe9; continue;
            }
            0x323fe9 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x323ffe; continue;
            }
            0x323fee => {
                ecx = m.read32(esp.wrapping_add(0x88));
                push!(edi);
                push!(ecx);
                push!(ebp);
                push!(esi);
                call_to!(eax);
                pc = 0x323ffb; continue;
            }
            0x323ffb => {
                esp = esp.wrapping_add(0x10);
                pc = 0x323ffe; continue;
            }
            0x323ffe => {
                eax = m.read32(esp.wrapping_add(0xa8));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x324010; continue; }
                pc = 0x324009; continue;
            }
            0x324009 => {
                push!(edi);
                push!(esi);
                call_to!(eax);
                pc = 0x32400d; continue;
            }
            0x32400d => {
                esp = esp.wrapping_add(0x8);
                pc = 0x324010; continue;
            }
            0x324010 => {
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x324011; continue;
            }
            0x324011 => {
                push!(0x3f800000);
                push!(0x3f800000);
                push!(0x3f800000);
                call!(0x3f9c0);
                pc = 0x324025; continue;
            }
            0x324025 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                esp = esp.wrapping_add(0xc);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x70);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x003244E0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3244e0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3244e0;
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
        { crate::pose::cover(0x3244e0, previous, pc); previous = pc; }
        match pc {
            0x3244e0 => {
                esp = esp.wrapping_sub(0x1c);
                push!(ebx);
                push!(esi);
                push!(edi);
                esi = eax;
                call!(0x31f220);
                pc = 0x3244ed; continue;
            }
            0x3244ed => {
                fpu.push((f32::from_bits(m.read32(0x7ba188)) as f64));
                edi = m.read32(esp.wrapping_add(0x30));
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(edi.wrapping_add(0x4))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                xmm0 = Xmm::scalar(m.read32(0x7ba038));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x47cfe0))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi))).to_bits();
                m.write32(esp.wrapping_add(0xc), (fpu.st(0) as f32).to_bits()); fpu.pop();
                ebx = eax;
                fpu.push((f32::from_bits(m.read32(0x7ba188)) as f64));
                ecx = m.read32(esp.wrapping_add(0xc));
                { let value = round_pc(fpu.st(0) + fpu.st(0), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x24), ebx);
                m.write32(esp.wrapping_add(0x20), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x18), (fpu.st(0) as f32).to_bits()); fpu.pop();
                eax = m.read32(esp.wrapping_add(0x18));
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x30));
                push!(ecx);
                push!(esi);
                call!(0x324370);
                pc = 0x32453e; continue;
            }
            0x32453e => {
                esp = esp.wrapping_add(0xc);
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x1c), eax);
                m.write32(0x763080, esi);
                m.write32(esp.wrapping_add(0x10), 0);
                if (fas <= fbs) { pc = 0x324663; continue; }
                pc = 0x32455b; continue;
            }
            0x32455b => {
                push!(ebp);
                pc = 0x324560; continue;
            }
            0x324560 => {
                esi = m.read32(0x7844a8);
                edx = m.read32(esi.wrapping_add(0x81a4));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x10)));
                ecx = 0;
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x18), xmm0.lanes[0]);
                if (fas <= fbs) { pc = 0x3245a2; continue; }
                pc = 0x32457e; continue;
            }
            0x32457e => {
                edi = m.read32(esp.wrapping_add(0x14));
                edi = edi.wrapping_add(1);
                eax = esi.wrapping_add(0x81a8);
                pc = 0x324590; continue;
            }
            0x324590 => {
                fa = m.read32(eax.wrapping_add(0xac));
                fb = edi;
                if (fa == fb) { pc = 0x3245a4; continue; }
                pc = 0x324598; continue;
            }
            0x324598 => {
                ecx = ecx.wrapping_add(1);
                eax = eax.wrapping_add(0xb4);
                fa = ecx;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x324590; continue; }
                pc = 0x3245a2; continue;
            }
            0x3245a2 => {
                eax = 0;
                pc = 0x3245a4; continue;
            }
            0x3245a4 => {
                edx = m.read32(0x762f98);
                edi = m.read32(esi.wrapping_add(0x248));
                ebx = 0;
                fa = eax;
                fb = edx;
                ebx = (ebx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3245c7; continue; }
                pc = 0x3245bb; continue;
            }
            0x3245bb => {
                edx = edi.wrapping_add(edi.wrapping_mul(0x8));
                ebp = m.read32(edx.wrapping_mul(0x4).wrapping_add(0x52e828));
                pc = 0x3245d1; continue;
            }
            0x3245c7 => {
                ecx = edi.wrapping_add(edi.wrapping_mul(0x8));
                ebp = m.read32(ecx.wrapping_mul(0x4).wrapping_add(0x52e830));
                pc = 0x3245d1; continue;
            }
            0x3245d1 => {
                edx = m.read32(esp.wrapping_add(0x20));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x324600; continue; }
                pc = 0x3245d9; continue;
            }
            0x3245d9 => {
                esi = ((m.read8(esi.wrapping_add(0x41dc))) as u8 as u32);
                ecx = 0;
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x3245f3; continue; }
                pc = 0x3245e6; continue;
            }
            0x3245e6 => {
                fa = m.read32(edx.wrapping_add(0x18));
                fb = eax;
                if (fa == fb) { pc = 0x3245f5; continue; }
                pc = 0x3245eb; continue;
            }
            0x3245eb => {
                ecx = ecx.wrapping_add(1);
                edx = edx.wrapping_add(0x20);
                fa = ecx;
                fb = esi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x3245e6; continue; }
                pc = 0x3245f3; continue;
            }
            0x3245f3 => {
                edx = 0;
                pc = 0x3245f5; continue;
            }
            0x3245f5 => {
                xmm0 = Xmm::scalar(m.read32(edx.wrapping_add(0x4)));
                m.write32(esp.wrapping_add(0x18), xmm0.lanes[0]);
                pc = 0x324600; continue;
            }
            0x324600 => {
                edx = m.read32(esp.wrapping_add(0x18));
                ecx = m.read32(esp.wrapping_add(0x24));
                push!(0x31ecd0);
                push!(0);
                xmm0 = Xmm::default();
                push!(ebp);
                push!(0x1);
                push!(0x1);
                push!(edx);
                edx = 0;
                eax = eax.wrapping_add(0x20);
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                edx = (edx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x4c));
                push!(0);
                push!(edi);
                edx = edx.wrapping_sub(1);
                edx = (edx & 0x2);
                push!(edx);
                call!(0x323c30);
                pc = 0x324634; continue;
            }
            0x324634 => {
                eax = m.read32(esp.wrapping_add(0x3c));
                ecx = m.read32(esp.wrapping_add(0x50));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x44)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x38)))).to_bits();
                esp = esp.wrapping_add(0x28);
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x14), eax);
                if (fas < fbs) { pc = 0x324560; continue; }
                pc = 0x32465e; continue;
            }
            0x32465e => {
                edi = m.read32(esp.wrapping_add(0x34));
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x324663; continue;
            }
            0x324663 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0xc)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x4)))).to_bits();
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x1c);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x003246A0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3246a0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3246a0;
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
        { crate::pose::cover(0x3246a0, previous, pc); previous = pc; }
        match pc {
            0x3246a0 => {
                push!(ecx);
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0xc))) as f64));
                ecx = m.read32(esp.wrapping_add(0x10));
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(esp.wrapping_add(0x14))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(ebx);
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0x20));
                xmm0 = Xmm::default();
                m.write32(esp.wrapping_add(0x1c), (fpu.st(0) as f32).to_bits()); fpu.pop();
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x20));
                push!(0);
                push!(0);
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x20));
                push!(0x1);
                push!(0x1);
                push!(ecx);
                push!(edi);
                push!(0);
                push!(edx);
                push!(0x1);
                ecx = esi;
                call!(0x323c30);
                pc = 0x3246da; continue;
            }
            0x3246da => {
                eax = m.read32(0x763080);
                ebx = m.read32(esp.wrapping_add(0x40));
                esp = esp.wrapping_add(0x28);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                if (fa == fb) { pc = 0x324743; continue; }
                pc = 0x3246f0; continue;
            }
            0x3246f0 => {
                eax = m.read32(0x763084);
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475cd4))).to_bits();
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_mul(0x4).wrapping_add(0x763074)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x1c)))).to_bits();
                ecx = esp.wrapping_add(0x18);
                push!(ecx);
                edx = esp.wrapping_add(0x24);
                push!(edx);
                m.write32(esp.wrapping_add(0x28), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x20), xmm0.lanes[0]);
                call!(0x30d410);
                pc = 0x324727; continue;
            }
            0x324727 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x324743; continue; }
                pc = 0x32472e; continue;
            }
            0x32472e => {
                eax = m.read32(esp.wrapping_add(0x18));
                ecx = m.read32(esp.wrapping_add(0x20));
                push!(ebp);
                push!(eax);
                push!(ebx);
                push!(ecx);
                push!(ebx);
                call!(0x1afa0);
                pc = 0x324740; continue;
            }
            0x324740 => {
                esp = esp.wrapping_add(0x14);
                pc = 0x324743; continue;
            }
            0x324743 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0xc)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475cd4))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x1c)))).to_bits();
                edx = m.read32(0x763084);
                push!(ebp);
                push!(edi);
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                eax = m.read32(esp.wrapping_add(0x24));
                push!(eax);
                push!(ebx);
                push!(0);
                push!(0);
                push!(0);
                push!(esi);
                m.write32(edx.wrapping_mul(0x4).wrapping_add(0x763074), xmm0.lanes[0]);
                call!(0x31ee30);
                pc = 0x324780; continue;
            }
            0x324780 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x2c)));
                esp = esp.wrapping_add(0x20);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
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

/// 0x00324C00: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f324c00<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x324c00;
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
        { crate::pose::cover(0x324c00, previous, pc); previous = pc; }
        match pc {
            0x324c00 => {
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x8));
                push!(0x2);
                call!(0x31f950);
                pc = 0x324c0c; continue;
            }
            0x324c0c => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x324c8b; continue; }
                pc = 0x324c13; continue;
            }
            0x324c13 => {
                push!(ebx);
                push!(0x1);
                push!(edi);
                call!(0x71880);
                pc = 0x324c1c; continue;
            }
            0x324c1c => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x324c54; continue; }
                pc = 0x324c23; continue;
            }
            0x324c23 => {
                eax = m.read32(0x7844a8);
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0x238))) as u32)) & 0xff);
                ebx = 0x20;
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((((ecx) as u8))) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x324c54; continue; }
                pc = 0x324c37; continue;
            }
            0x324c37 => {
                push!(0x17);
                call!(0x3370a0);
                pc = 0x324c3e; continue;
            }
            0x324c3e => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x238));
                esp = esp.wrapping_add(0x4);
                ecx = (ecx | ebx);
                m.write32(eax.wrapping_add(0x238), ecx);
                pc = 0x324c54; continue;
            }
            0x324c54 => {
                ebx = edi;
                call!(0x324b60);
                pc = 0x324c5b; continue;
            }
            0x324c5b => {
                m.write32(0x783a1c, 0x1);
                m.write32(0x77d840, 0x1);
                ecx = m.read32(edi.wrapping_add(0x2c));
                push!(0x52ea18);
                push!(ecx);
                m.write32(0x76307c, 0x2);
                call!(0x79b70);
                pc = 0x324c87; continue;
            }
            0x324c87 => {
                esp = esp.wrapping_add(0x8);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x324c8b; continue;
            }
            0x324c8b => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x003266B0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3266b0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3266b0;
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
        { crate::pose::cover(0x3266b0, previous, pc); previous = pc; }
        match pc {
            0x3266b0 => {
                push!(ecx);
                fa = eax;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                push!(ebx);
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0x20));
                push!(esi);
                if (fa == fb) { pc = 0x3266c5; continue; }
                pc = 0x3266bd; continue;
            }
            0x3266bd => {
                fa = eax;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                eax = ecx.wrapping_add(0x20);
                if (fa != fb) { pc = 0x3266c8; continue; }
                pc = 0x3266c5; continue;
            }
            0x3266c5 => {
                eax = ecx.wrapping_add(0x60);
                pc = 0x3266c8; continue;
            }
            0x3266c8 => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x18))) as f64));
                xmm0 = Xmm::default();
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(esp.wrapping_add(0x20))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(0x31ecd0);
                push!(0x326480);
                push!(edx);
                edx = m.read32(esp.wrapping_add(0x28));
                m.write32(esp.wrapping_add(0x2c), (fpu.st(0) as f32).to_bits()); fpu.pop();
                esi = m.read32(esp.wrapping_add(0x2c));
                push!(0x1);
                push!(0x1);
                push!(edx);
                push!(esi);
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x34));
                push!(ecx);
                push!(0x1);
                ecx = edi;
                call!(0x323c30);
                pc = 0x3266ff; continue;
            }
            0x3266ff => {
                eax = m.read32(0x763080);
                ebx = m.read32(esp.wrapping_add(0x40));
                esp = esp.wrapping_add(0x28);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                if (fa == fb) { pc = 0x326769; continue; }
                pc = 0x326715; continue;
            }
            0x326715 => {
                edx = m.read32(0x763084);
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475cd4))).to_bits();
                xmm1 = Xmm::scalar(m.read32(edx.wrapping_mul(0x4).wrapping_add(0x763074)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x1c)))).to_bits();
                eax = esp.wrapping_add(0x18);
                push!(eax);
                ecx = esp.wrapping_add(0x24);
                push!(ecx);
                m.write32(esp.wrapping_add(0x28), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x20), xmm0.lanes[0]);
                call!(0x30d410);
                pc = 0x32674d; continue;
            }
            0x32674d => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x326769; continue; }
                pc = 0x326754; continue;
            }
            0x326754 => {
                edx = m.read32(esp.wrapping_add(0x18));
                eax = m.read32(esp.wrapping_add(0x20));
                push!(ebp);
                push!(edx);
                push!(ebx);
                push!(eax);
                push!(ebx);
                call!(0x1afa0);
                pc = 0x326766; continue;
            }
            0x326766 => {
                esp = esp.wrapping_add(0x14);
                pc = 0x326769; continue;
            }
            0x326769 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0xc)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475cd4))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x1c)))).to_bits();
                ecx = m.read32(0x763084);
                push!(ebp);
                push!(esi);
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                edx = m.read32(esp.wrapping_add(0x24));
                push!(edx);
                push!(ebx);
                push!(0);
                push!(0);
                push!(0);
                push!(edi);
                m.write32(ecx.wrapping_mul(0x4).wrapping_add(0x763074), xmm0.lanes[0]);
                call!(0x31ee30);
                pc = 0x3267a6; continue;
            }
            0x3267a6 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x2c)));
                esp = esp.wrapping_add(0x20);
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

/// 0x003267C0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3267c0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3267c0;
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
        { crate::pose::cover(0x3267c0, previous, pc); previous = pc; }
        match pc {
            0x3267c0 => {
                push!(ecx);
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0xc))) as f64));
                edx = m.read32(esp.wrapping_add(0x10));
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(esp.wrapping_add(0x14))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(ebx);
                push!(ebp);
                push!(edi);
                ebp = m.read32(esp.wrapping_add(0x24));
                m.write32(esp.wrapping_add(0x20), (fpu.st(0) as f32).to_bits()); fpu.pop();
                edi = m.read32(esp.wrapping_add(0x20));
                xmm0 = Xmm::default();
                push!(0x31ecd0);
                push!(0x326480);
                push!(eax);
                push!(0x1);
                push!(0x1);
                push!(edx);
                push!(edi);
                push!(ecx);
                eax = ecx.wrapping_add(0x20);
                ecx = m.read32(esp.wrapping_add(0x34));
                push!(ecx);
                push!(0x2);
                ecx = esi;
                call!(0x323c30);
                pc = 0x326802; continue;
            }
            0x326802 => {
                eax = m.read32(0x763080);
                ebx = m.read32(esp.wrapping_add(0x40));
                esp = esp.wrapping_add(0x28);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                if (fa == fb) { pc = 0x32686c; continue; }
                pc = 0x326818; continue;
            }
            0x326818 => {
                edx = m.read32(0x763084);
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475cd4))).to_bits();
                xmm1 = Xmm::scalar(m.read32(edx.wrapping_mul(0x4).wrapping_add(0x763074)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x1c)))).to_bits();
                eax = esp.wrapping_add(0x18);
                push!(eax);
                ecx = esp.wrapping_add(0x24);
                push!(ecx);
                m.write32(esp.wrapping_add(0x28), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x20), xmm0.lanes[0]);
                call!(0x30d410);
                pc = 0x326850; continue;
            }
            0x326850 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x32686c; continue; }
                pc = 0x326857; continue;
            }
            0x326857 => {
                edx = m.read32(esp.wrapping_add(0x18));
                eax = m.read32(esp.wrapping_add(0x20));
                push!(ebp);
                push!(edx);
                push!(ebx);
                push!(eax);
                push!(ebx);
                call!(0x1afa0);
                pc = 0x326869; continue;
            }
            0x326869 => {
                esp = esp.wrapping_add(0x14);
                pc = 0x32686c; continue;
            }
            0x32686c => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0xc)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475cd4))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x1c)))).to_bits();
                ecx = m.read32(0x763084);
                push!(ebp);
                push!(edi);
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                edx = m.read32(esp.wrapping_add(0x24));
                push!(edx);
                push!(ebx);
                push!(0);
                push!(0);
                push!(0);
                push!(esi);
                m.write32(ecx.wrapping_mul(0x4).wrapping_add(0x763074), xmm0.lanes[0]);
                call!(0x31ee30);
                pc = 0x3268a9; continue;
            }
            0x3268a9 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x2c)));
                esp = esp.wrapping_add(0x20);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
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

/// 0x003268C0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3268c0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3268c0;
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
        { crate::pose::cover(0x3268c0, previous, pc); previous = pc; }
        match pc {
            0x3268c0 => {
                eax = m.read32(0x7844a8);
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x4))) as f64));
                eax = m.read32(eax.wrapping_add(0x244));
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(esp.wrapping_add(0x8))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(ebx);
                push!(0x31ecd0);
                ecx = eax.wrapping_add(esi.wrapping_mul(0x8));
                eax = ((eax as i32).wrapping_mul((0xd) as i32)) as u32;
                m.write32(esp.wrapping_add(0x10), (fpu.st(0) as f32).to_bits()); fpu.pop();
                ebx = m.read32(esp.wrapping_add(0x10));
                push!(0x326550);
                ecx = ecx.wrapping_add(esi);
                edx = m.read32(ecx.wrapping_mul(0x4).wrapping_add(0x52e818));
                ecx = m.read32(esp.wrapping_add(0x18));
                push!(edx);
                push!(0x1);
                push!(0);
                push!(ecx);
                eax = eax.wrapping_add(esi);
                edx = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x52e6bc));
                push!(ebx);
                push!(edx);
                call!(0x3d7b0);
                pc = 0x32690f; continue;
            }
            0x32690f => {
                ecx = m.read32(0x7844a8);
                edx = m.read32(ecx.wrapping_add(0x244));
                xmm0 = Xmm::default();
                esp = esp.wrapping_add(0x4);
                push!(0);
                push!(esi);
                push!(edx);
                ecx = edi;
                call!(0x323c30);
                pc = 0x32692c; continue;
            }
            0x32692c => {
                ecx = m.read32(esp.wrapping_add(0x3c));
                eax = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                eax = m.read32(0x763084);
                push!(ecx);
                m.write32(esp.wrapping_add(0x38), xmm0.lanes[0]);
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475cd4))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x3c)))).to_bits();
                push!(ebx);
                m.write32(esp.wrapping_add(0x40), xmm0.lanes[0]);
                edx = m.read32(esp.wrapping_add(0x40));
                push!(edx);
                m.write32(eax.wrapping_mul(0x4).wrapping_add(0x763074), xmm0.lanes[0]);
                eax = m.read32(esp.wrapping_add(0x3c));
                push!(eax);
                push!(0);
                push!(0);
                push!(0);
                push!(edi);
                call!(0x31ee30);
                pc = 0x326978; continue;
            }
            0x326978 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x54)));
                esp = esp.wrapping_add(0x48);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x003276F0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3276f0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3276f0;
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
        { crate::pose::cover(0x3276f0, previous, pc); previous = pc; }
        match pc {
            0x3276f0 => {
                push!(ecx);
                push!(ebx);
                push!(edi);
                ebx = eax;
                call!(0x711f0);
                pc = 0x3276fa; continue;
            }
            0x3276fa => {
                xmm0 = Xmm::scalar(m.read32(0x475d20));
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x327711; continue; }
                pc = 0x327709; continue;
            }
            0x327709 => {
                xmm0 = Xmm::scalar(m.read32(0x475cc8));
                pc = 0x327711; continue;
            }
            0x327711 => {
                eax = m.read32(esp.wrapping_add(0x10));
                ecx = m.read32(eax);
                push!(ebp);
                m.write32(esp.wrapping_add(0x8), xmm0.lanes[0]);
                push!(esi);
                esi = m.read32(esp.wrapping_add(0xc));
                push!(esi);
                push!(ecx);
                call!(0x154e90);
                pc = 0x32772a; continue;
            }
            0x32772a => {
                edx = m.read32(esp.wrapping_add(0x20));
                ebp = eax;
                eax = m.read32(edx.wrapping_add(0x4));
                push!(esi);
                push!(eax);
                call!(0x154e90);
                pc = 0x32773a; continue;
            }
            0x32773a => {
                ecx = m.read32(ebx.wrapping_add(0xf4));
                push!(ecx);
                push!(ebp);
                m.write32(esp.wrapping_add(0x30), eax);
                call!(0x154fe0);
                pc = 0x32774b; continue;
            }
            0x32774b => {
                edx = m.read32(ebx.wrapping_add(0xf4));
                esi = eax;
                eax = m.read32(esp.wrapping_add(0x30));
                push!(edx);
                push!(eax);
                call!(0x154fe0);
                pc = 0x32775e; continue;
            }
            0x32775e => {
                ecx = eax;
                edx = eax;
                edx = edx.wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx & 0xff0000);
                ecx = (ecx | edx);
                edx = eax;
                edx = (edx & 0xff00);
                eax = eax.wrapping_shl((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = (edx | eax);
                edx = edx.wrapping_shl((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = ecx.wrapping_shr((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx | edx);
                eax = esi;
                eax = (eax & 0xff0000);
                edx = esi;
                edx = edx.wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = (eax | edx);
                push!(ecx);
                edx = esi;
                edx = (edx & 0xff00);
                push!(ecx);
                ecx = m.read32(edi.wrapping_add(0x8));
                esi = esi.wrapping_shl((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = (edx | esi);
                eax = eax.wrapping_shr((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = edx.wrapping_shl((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = (eax | edx);
                edx = m.read32(edi.wrapping_add(0x4));
                push!(eax);
                push!(eax);
                eax = m.read32(edi.wrapping_add(0xc));
                push!(eax);
                eax = m.read32(edi);
                push!(ecx);
                push!(edx);
                push!(eax);
                call!(0x1a6f0);
                pc = 0x3277bd; continue;
            }
            0x3277bd => {
                eax = m.read32(esp.wrapping_add(0x5c));
                esp = esp.wrapping_add(0x40);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                if (fa == fb) { pc = 0x327824; continue; }
                pc = 0x3277ca; continue;
            }
            0x3277ca => {
                xmm0 = Xmm::scalar(m.read32(ebx.wrapping_add(0x1b8)));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x475c78)) as f64);
                eax = (((eax & 0xffff00ffu32)) | ((((((if ((((fca == fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x40u32 } else { 0x0u32 }) | (if ((((fca != fca) || (fcb != fcb)))) { 0x4u32 } else { 0x0u32 })) | (if ((((fca < fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x1u32 } else { 0x0u32 })) | 0x2u32)).wrapping_shl((0x8 & 31))));
                fa = (((((((eax) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x44i32)) as u32) & 0xffu32);
                if (((((fa) & (fb))) as u8).count_ones() % 2 == 0) { pc = 0x327824; continue; }
                pc = 0x3277df; continue;
            }
            0x3277df => {
                ecx = m.read32(esp.wrapping_add(0xc));
                ebx = m.read32(ecx.wrapping_add(0x14));
                call!(0x326630);
                pc = 0x3277eb; continue;
            }
            0x3277eb => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x327824; continue; }
                pc = 0x3277ef; continue;
            }
            0x3277ef => {
                xmm0 = Xmm::scalar(m.read32(edi.wrapping_add(0x4)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475db4))).to_bits();
                push!(0x3f800000);
                esp = esp.wrapping_sub(0x8);
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(edi.wrapping_add(0x8)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x47d1e4))).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                call!(0x31eb40);
                pc = 0x327821; continue;
            }
            0x327821 => {
                esp = esp.wrapping_add(0xc);
                pc = 0x327824; continue;
            }
            0x327824 => {
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00327A70: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f327a70<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x327a70;
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
        { crate::pose::cover(0x327a70, previous, pc); previous = pc; }
        match pc {
            0x327a70 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x248));
                esp = esp.wrapping_sub(0x8);
                push!(ebx);
                push!(ebp);
                push!(esi);
                esi = m.read32(0x76306c);
                ecx = esi.wrapping_add(ecx.wrapping_mul(0x4));
                edx = ecx.wrapping_add(ecx.wrapping_mul(0x2));
                ebx = eax.wrapping_add(edx.wrapping_mul(0x8)).wrapping_add(0x11394);
                m.write32(eax.wrapping_add(0x11484), 0);
                edx = 0;
                push!(edi);
                ecx = 0;
                eax = 0x52e948;
                pc = 0x327aa8; continue;
            }
            0x327aa8 => {
                fa = m.read32(eax);
                fb = esi;
                if (fa == fb) { pc = 0x327ab9; continue; }
                pc = 0x327aac; continue;
            }
            0x327aac => {
                eax = eax.wrapping_add(0x20);
                ecx = ecx.wrapping_add(1);
                fa = eax;
                fb = ((((0x52e9c8i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x327aa8; continue; }
                pc = 0x327ab7; continue;
            }
            0x327ab7 => {
                pc = 0x327ac4; continue;
            }
            0x327ab9 => {
                ecx = ecx.wrapping_shl((((((((0x5i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = ecx.wrapping_add(0x52e948);
                edx = ecx;
                pc = 0x327ac4; continue;
            }
            0x327ac4 => {
                eax = m.read32(edx.wrapping_add(0x4));
                push!(eax);
                call!(0x3d7b0);
                pc = 0x327acd; continue;
            }
            0x327acd => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0xb48);
                call!(0x3d7b0);
                pc = 0x327adb; continue;
            }
            0x327adb => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                eax = m.read32(0x7844a8);
                push!(eax);
                call!(0x3c85d4);
                pc = 0x327aea; continue;
            }
            0x327aea => {
                esi = m.read32(0x76306c);
                esp = esp.wrapping_add(0xc);
                edx = 0;
                ecx = 0;
                eax = 0x52e948;
                pc = 0x327b00; continue;
            }
            0x327b00 => {
                fa = m.read32(eax);
                fb = esi;
                if (fa == fb) { pc = 0x327b11; continue; }
                pc = 0x327b04; continue;
            }
            0x327b04 => {
                eax = eax.wrapping_add(0x20);
                ecx = ecx.wrapping_add(1);
                fa = eax;
                fb = ((((0x52e9c8i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x327b00; continue; }
                pc = 0x327b0f; continue;
            }
            0x327b0f => {
                pc = 0x327b1c; continue;
            }
            0x327b11 => {
                ecx = ecx.wrapping_shl((((((((0x5i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = ecx.wrapping_add(0x52e948);
                edx = ecx;
                pc = 0x327b1c; continue;
            }
            0x327b1c => {
                eax = m.read32(edx.wrapping_add(0x10));
                push!(0x361710);
                push!(0);
                push!(0x42480000);
                push!(eax);
                call!(0x3d7b0);
                pc = 0x327b31; continue;
            }
            0x327b31 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x8400);
                call!(0x75b70);
                pc = 0x327b3f; continue;
            }
            0x327b3f => {
                push!(0x1);
                push!(0x1);
                push!(0x40000000);
                push!(0x1);
                call!(0x71b10);
                pc = 0x327b4f; continue;
            }
            0x327b4f => {
                push!(0x28000);
                call!(0x71c50);
                pc = 0x327b59; continue;
            }
            0x327b59 => {
                esp = esp.wrapping_add(0x28);
                push!(0);
                push!(0);
                push!(0);
                call!(0x22b90);
                pc = 0x327b67; continue;
            }
            0x327b67 => {
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
                pc = 0x327bb5; continue;
            }
            0x327bb5 => {
                esp = esp.wrapping_add(0x10);
                push!(0);
                push!(0x31f700);
                push!(0);
                push!(0xc00);
                push!(0xffffffffu32);
                push!(0xb44);
                call!(0x3d7b0);
                pc = 0x327bd2; continue;
            }
            0x327bd2 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x327bdb; continue;
            }
            0x327bdb => {
                push!(0x10c);
                call!(0x716d0);
                pc = 0x327be5; continue;
            }
            0x327be5 => {
                push!(0x40000000);
                call!(0x784a0);
                pc = 0x327bef; continue;
            }
            0x327bef => {
                push!(0x31f6e0);
                push!(0x31fb50);
                call!(0x324a00);
                pc = 0x327bfe; continue;
            }
            0x327bfe => {
                push!(0xb49);
                ebp = eax;
                call!(0x3d7b0);
                pc = 0x327c0a; continue;
            }
            0x327c0a => {
                push!(0);
                push!(0);
                push!(0);
                push!(0x800);
                push!(0xffffffffu32);
                push!(eax);
                call!(0x79d60);
                pc = 0x327c1d; continue;
            }
            0x327c1d => {
                esp = esp.wrapping_add(0x44);
                push!(0);
                push!(0);
                call!(0x7d4f0);
                pc = 0x327c29; continue;
            }
            0x327c29 => {
                push!(0x10d);
                call!(0x716d0);
                pc = 0x327c33; continue;
            }
            0x327c33 => {
                esi = ebx.wrapping_add(0x8);
                push!(esi);
                call!(0x78560);
                pc = 0x327c3c; continue;
            }
            0x327c3c => {
                push!(0xb4a);
                call!(0x3d7b0);
                pc = 0x327c46; continue;
            }
            0x327c46 => {
                push!(0);
                push!(0);
                push!(0);
                push!(0x800);
                push!(0xffffffffu32);
                push!(eax);
                call!(0x79d60);
                pc = 0x327c59; continue;
            }
            0x327c59 => {
                push!(0);
                push!(0);
                call!(0x7d4f0);
                pc = 0x327c62; continue;
            }
            0x327c62 => {
                push!(0x10e);
                call!(0x716d0);
                pc = 0x327c6c; continue;
            }
            0x327c6c => {
                edi = ebx.wrapping_add(0xc);
                push!(edi);
                call!(0x78560);
                pc = 0x327c75; continue;
            }
            0x327c75 => {
                push!(0x40000000);
                call!(0x784a0);
                pc = 0x327c7f; continue;
            }
            0x327c7f => {
                esp = esp.wrapping_add(0x40);
                push!(0xb4b);
                call!(0x3d7b0);
                pc = 0x327c8c; continue;
            }
            0x327c8c => {
                push!(0xb4c);
                m.write32(esp.wrapping_add(0x1c), eax);
                call!(0x3d7b0);
                pc = 0x327c9a; continue;
            }
            0x327c9a => {
                push!(0);
                push!(0);
                push!(0);
                push!(0x4900);
                push!(0xffffffffu32);
                push!(0);
                m.write32(esp.wrapping_add(0x30), eax);
                call!(0x79d60);
                pc = 0x327cb2; continue;
            }
            0x327cb2 => {
                push!(0);
                push!(0x900);
                call!(0x7d4a0);
                pc = 0x327cbe; continue;
            }
            0x327cbe => {
                push!(0x10f);
                call!(0x716d0);
                pc = 0x327cc8; continue;
            }
            0x327cc8 => {
                ecx = m.read32(esp.wrapping_add(0x3c));
                push!(ecx);
                call!(0x717d0);
                pc = 0x327cd2; continue;
            }
            0x327cd2 => {
                edx = m.read32(esp.wrapping_add(0x44));
                push!(edx);
                call!(0x717d0);
                pc = 0x327cdc; continue;
            }
            0x327cdc => {
                ebx = ebx.wrapping_add(0x10);
                push!(ebx);
                call!(0x78560);
                pc = 0x327ce5; continue;
            }
            0x327ce5 => {
                ebx = m.read32(edi);
                edi = m.read32(esi);
                eax = 0;
                esp = esp.wrapping_add(0x38);
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = (eax & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                ecx = 0;
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ecx = (ecx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                edx = 0;
                ecx = ecx.wrapping_add(ebp);
                esi = ecx.wrapping_add(eax);
                fa = esi;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                edx = (edx & 0xffff_ff00) | ((((if ((fas <= fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                push!(edx);
                call!(0x71630);
                pc = 0x327d0d; continue;
            }
            0x327d0d => {
                push!(eax);
                call!(0x71c70);
                pc = 0x327d13; continue;
            }
            0x327d13 => {
                esp = esp.wrapping_add(0x8);
                push!(0);
                push!(0x361710);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0xb5e);
                call!(0x3d7b0);
                pc = 0x327d2d; continue;
            }
            0x327d2d => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x327d36; continue;
            }
            0x327d36 => {
                eax = 0;
                esp = esp.wrapping_add(0x18);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = (eax & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                push!(eax);
                call!(0x71630);
                pc = 0x327d46; continue;
            }
            0x327d46 => {
                push!(eax);
                call!(0x71c70);
                pc = 0x327d4c; continue;
            }
            0x327d4c => {
                call!(0x781a0);
                pc = 0x327d51; continue;
            }
            0x327d51 => {
                call!(0x22b90);
                pc = 0x327d56; continue;
            }
            0x327d56 => {
                xmm0 = Xmm::scalar(m.read32(0x7ba188));
                xmm1.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm2 = xmm0;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x475d24))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d30))).to_bits();
                ecx = m.read32(0x7bb1e8);
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x7ba184))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x7ba030))).to_bits();
                esp = esp.wrapping_add(0x4);
                m.write32(esp, xmm1.lanes[0]);
                push!(ecx);
                call!(0x77320);
                pc = 0x327da1; continue;
            }
            0x327da1 => {
                esp = esp.wrapping_add(0x8);
                call!(0x75e70);
                pc = 0x327da9; continue;
            }
            0x327da9 => {
                eax = m.read32(esp.wrapping_add(0x1c));
                ecx = m.read32(eax.wrapping_add(0x218));
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = (ecx | 0x4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(eax.wrapping_add(0x218), ecx);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00327F90: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f327f90<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x327f90;
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
        { crate::pose::cover(0x327f90, previous, pc); previous = pc; }
        match pc {
            0x327f90 => {
                eax = m.read32(0x762f98);
                ecx = m.read32(eax.wrapping_add(0xa4));
                eax = m.read32(0x7844a8);
                ecx = ((ecx as i32).wrapping_mul((0x220) as i32)) as u32;
                push!(ebx);
                push!(esi);
                esi = m.read32(eax.wrapping_add(0x248));
                push!(edi);
                esi = ((esi as i32).wrapping_mul((0x44) as i32)) as u32;
                push!(0x327eb0);
                push!(0);
                push!(0x42480000);
                push!(0xd68);
                ebx = ecx.wrapping_add(eax).wrapping_add(0x41e4);
                esi = esi.wrapping_add(0x52e8c0);
                call!(0x3d7b0);
                pc = 0x327fd5; continue;
            }
            0x327fd5 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x8400);
                call!(0x75b70);
                pc = 0x327fe3; continue;
            }
            0x327fe3 => {
                push!(0x1);
                push!(0x1);
                push!(0x40000000);
                push!(0x1);
                call!(0x71b10);
                pc = 0x327ff3; continue;
            }
            0x327ff3 => {
                push!(0x28000);
                call!(0x71c50);
                pc = 0x327ffd; continue;
            }
            0x327ffd => {
                esp = esp.wrapping_add(0x28);
                push!(0);
                push!(0);
                push!(0);
                call!(0x22b90);
                pc = 0x32800b; continue;
            }
            0x32800b => {
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
                pc = 0x328059; continue;
            }
            0x328059 => {
                esp = esp.wrapping_add(0x10);
                push!(0);
                push!(0x324870);
                push!(0);
                push!(0x800);
                push!(0xffffffffu32);
                push!(0xb44);
                call!(0x3d7b0);
                pc = 0x328076; continue;
            }
            0x328076 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x32807f; continue;
            }
            0x32807f => {
                push!(0xd8);
                call!(0x716d0);
                pc = 0x328089; continue;
            }
            0x328089 => {
                push!(0x40000000);
                call!(0x784a0);
                pc = 0x328093; continue;
            }
            0x328093 => {
                push!(0x31f7d0);
                push!(0x31fb70);
                call!(0x324a00);
                pc = 0x3280a2; continue;
            }
            0x3280a2 => {
                push!(0x40000000);
                edi = eax;
                call!(0x784a0);
                pc = 0x3280ae; continue;
            }
            0x3280ae => {
                push!(0);
                push!(0);
                push!(0);
                push!(0x4900);
                push!(0);
                push!(0);
                call!(0x79d60);
                pc = 0x3280c2; continue;
            }
            0x3280c2 => {
                esp = esp.wrapping_add(0x44);
                ecx = 0x5;
                push!(0);
                push!(0);
                m.write32(eax.wrapping_add(0x1c), ecx);
                m.write32(eax.wrapping_add(0xbc), ecx);
                call!(0x7d4a0);
                pc = 0x3280dc; continue;
            }
            0x3280dc => {
                push!(0xda);
                call!(0x716d0);
                pc = 0x3280e6; continue;
            }
            0x3280e6 => {
                push!(0xd6b);
                call!(0x3d7b0);
                pc = 0x3280f0; continue;
            }
            0x3280f0 => {
                push!(eax);
                call!(0x717d0);
                pc = 0x3280f6; continue;
            }
            0x3280f6 => {
                push!(0xd6c);
                call!(0x3d7b0);
                pc = 0x328100; continue;
            }
            0x328100 => {
                push!(eax);
                call!(0x717d0);
                pc = 0x328106; continue;
            }
            0x328106 => {
                ebx = ebx.wrapping_add(0x4c);
                push!(ebx);
                call!(0x78560);
                pc = 0x32810f; continue;
            }
            0x32810f => {
                edx = 0;
                esp = esp.wrapping_add(0x20);
                fa = edi;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                edx = (edx & 0xffff_ff00) | ((((if ((fas <= fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                push!(edx);
                call!(0x71630);
                pc = 0x328120; continue;
            }
            0x328120 => {
                push!(eax);
                call!(0x71c70);
                pc = 0x328126; continue;
            }
            0x328126 => {
                eax = m.read32(0x778d3c);
                esp = esp.wrapping_add(0x8);
                push!(0);
                push!(0x327dd0);
                push!(0);
                cf = (((((eax) != 0))) as i32);
                eax = (((((eax) as i32).wrapping_neg())) as u32);
                push!(0);
                eax = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                eax = eax.wrapping_add(0xb5e);
                push!(0x1);
                push!(eax);
                call!(0x3d7b0);
                pc = 0x32814a; continue;
            }
            0x32814a => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x328153; continue;
            }
            0x328153 => {
                push!(0xdb);
                call!(0x716d0);
                pc = 0x32815d; continue;
            }
            0x32815d => {
                ecx = 0;
                esp = esp.wrapping_add(0x1c);
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ecx = (ecx & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                push!(ecx);
                call!(0x71630);
                pc = 0x32816d; continue;
            }
            0x32816d => {
                push!(eax);
                call!(0x71c70);
                pc = 0x328173; continue;
            }
            0x328173 => {
                call!(0x781a0);
                pc = 0x328178; continue;
            }
            0x328178 => {
                call!(0x22b90);
                pc = 0x32817d; continue;
            }
            0x32817d => {
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
                esp = esp.wrapping_add(0x4);
                m.write32(esp, xmm1.lanes[0]);
                push!(edx);
                call!(0x77320);
                pc = 0x3281c8; continue;
            }
            0x3281c8 => {
                eax = m.read32(esi.wrapping_add(0x4));
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                if (fa == fb) { pc = 0x3281f1; continue; }
                pc = 0x3281d5; continue;
            }
            0x3281d5 => {
                push!(0);
                push!(0);
                push!(0xb66);
                call!(0x3d7b0);
                pc = 0x3281e3; continue;
            }
            0x3281e3 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x3);
                call!(0x74a00);
                pc = 0x3281ee; continue;
            }
            0x3281ee => {
                esp = esp.wrapping_add(0x10);
                pc = 0x3281f1; continue;
            }
            0x3281f1 => {
                call!(0x75e70);
                pc = 0x3281f6; continue;
            }
            0x3281f6 => {
                eax = m.read32(esp.wrapping_add(0x4));
                m.write32(eax.wrapping_add(0x218), (m.read32(eax.wrapping_add(0x218)) | 0x4));
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00328D20: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f328d20<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x328d20;
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
        { crate::pose::cover(0x328d20, previous, pc); previous = pc; }
        match pc {
            0x328d20 => {
                esp = esp.wrapping_sub(0x1c);
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x11858));
                edx = m.read32(0x76307c);
                push!(ebx);
                push!(ebp);
                ebp = m.read32(ecx.wrapping_add(0x4));
                ebp = ((ebp as i32).wrapping_mul((0x4c) as i32)) as u32;
                ecx = 0;
                ebp = ebp.wrapping_add(0x52e0b8);
                fa = edx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                ecx = (ecx & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                fa = edx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                push!(esi);
                push!(edi);
                m.write32(esp.wrapping_add(0x18), ebp);
                ecx = ecx.wrapping_add(ecx).wrapping_add(0xd79);
                if (fa != fb) { pc = 0x328d61; continue; }
                pc = 0x328d5c; continue;
            }
            0x328d5c => {
                edx = m.read32(ebp.wrapping_add(0x10));
                pc = 0x328d64; continue;
            }
            0x328d61 => {
                edx = m.read32(ebp.wrapping_add(0x18));
                pc = 0x328d64; continue;
            }
            0x328d64 => {
                edi = m.read32(eax.wrapping_add(0x11858));
                m.write32(esp.wrapping_add(0x1c), edx);
                edx = (((((m.read16(edi.wrapping_add(0xc)) as i16)) as i32)) as u32);
                edx = ((edx as i32).wrapping_mul((0x8c) as i32)) as u32;
                push!(0x328bf0);
                push!(0);
                push!(0x425c0000);
                push!(ecx);
                m.write32(esp.wrapping_add(0x24), edi);
                esi = edx.wrapping_add(eax).wrapping_add(0xdd14);
                call!(0x3d7b0);
                pc = 0x328d95; continue;
            }
            0x328d95 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x8400);
                call!(0x75b70);
                pc = 0x328da3; continue;
            }
            0x328da3 => {
                push!(0x1);
                push!(0x1);
                push!(0);
                push!(0x1);
                call!(0x71b10);
                pc = 0x328db0; continue;
            }
            0x328db0 => {
                push!(0x20000);
                call!(0x71c50);
                pc = 0x328dba; continue;
            }
            0x328dba => {
                esp = esp.wrapping_add(0x28);
                push!(0);
                push!(0);
                push!(0);
                call!(0x22b90);
                pc = 0x328dc8; continue;
            }
            0x328dc8 => {
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
                pc = 0x328e16; continue;
            }
            0x328e16 => {
                edx = m.read32(0x76307c);
                esp = esp.wrapping_add(0x10);
                eax = 0;
                ebx = (ebx | 0xffffffffu32);
                fa = edx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                push!(0);
                eax = (eax & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                push!(0);
                push!(0);
                push!(ebx);
                eax = eax.wrapping_add(0xb68);
                push!(eax);
                call!(0x3d7b0);
                pc = 0x328e3c; continue;
            }
            0x328e3c => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79460);
                pc = 0x328e45; continue;
            }
            0x328e45 => {
                ecx = m.read32(0x7844a8);
                edx = m.read32(ecx.wrapping_add(0x11858));
                push!(0);
                push!(0);
                push!(0x40000000);
                push!(0x804);
                edx = edx.wrapping_add(0x20);
                push!(ebx);
                push!(edx);
                call!(0x79d60);
                pc = 0x328e69; continue;
            }
            0x328e69 => {
                esp = esp.wrapping_add(0x2c);
                call!(0x783d0);
                pc = 0x328e71; continue;
            }
            0x328e71 => {
                fa = ((((m.read8(ebp.wrapping_add(0x20)))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x328f3b; continue; }
                pc = 0x328e7b; continue;
            }
            0x328e7b => {
                eax = m.read32(0x7844a8);
                eax = m.read32(eax.wrapping_add(0x11858));
                ecx = m.read32(eax.wrapping_add(0x4));
                fa = ecx;
                fb = ((((0x9i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x328e93; continue; }
                pc = 0x328e8e; continue;
            }
            0x328e8e => {
                fa = ecx;
                fb = ((((0xci32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x328ea9; continue; }
                pc = 0x328e93; continue;
            }
            0x328e93 => {
                fa = ((((m.read16(eax.wrapping_add(0xc)))) as u32) & 0xffffu32);
                fb = ((((0i32)) as u32) & 0xffffu32);
                fas = (((((fa)) as i16)) as i32);
                fbs = (((((fb)) as i16)) as i32);
                if (fas >= fbs) { pc = 0x328f3b; continue; }
                pc = 0x328e9e; continue;
            }
            0x328e9e => {
                fa = ((((m.read16(eax.wrapping_add(0xe)))) as u32) & 0xffffu32);
                fb = ((((0i32)) as u32) & 0xffffu32);
                fas = (((((fa)) as i16)) as i32);
                fbs = (((((fb)) as i16)) as i32);
                if (fas >= fbs) { pc = 0x328f3b; continue; }
                pc = 0x328ea9; continue;
            }
            0x328ea9 => {
                ecx = m.read32(ebp);
                push!(0);
                push!(0);
                push!(0x800);
                push!(ebx);
                push!(ecx);
                call!(0x3d7b0);
                pc = 0x328ebc; continue;
            }
            0x328ebc => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x328ec5; continue;
            }
            0x328ec5 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x11858));
                esp = esp.wrapping_add(0x14);
                fa = ((((m.read16(ecx.wrapping_add(0xc)))) as u32) & 0xffffu32);
                fb = ((((((ebx) as u16))) as u32) & 0xffffu32);
                fas = (((((fa)) as i16)) as i32);
                fbs = (((((fb)) as i16)) as i32);
                if (fas <= fbs) { pc = 0x328eec; continue; }
                pc = 0x328ed9; continue;
            }
            0x328ed9 => {
                push!(0x2000);
                edx = esi.wrapping_add(0x78);
                push!(edx);
                push!(0x428c0000);
                push!(0x1f);
                push!(esi);
                pc = 0x328f1d; continue;
            }
            0x328eec => {
                fa = ((((m.read16(ecx.wrapping_add(0xe)))) as u32) & 0xffffu32);
                fb = ((((((ebx) as u16))) as u32) & 0xffffu32);
                fas = (((((fa)) as i16)) as i32);
                fbs = (((((fb)) as i16)) as i32);
                if (fas <= fbs) { pc = 0x328f29; continue; }
                pc = 0x328ef2; continue;
            }
            0x328ef2 => {
                ecx = m.read32(eax.wrapping_add(0x11858));
                edx = (((((m.read16(ecx.wrapping_add(0xe)) as i16)) as i32)) as u32);
                edx = ((edx as i32).wrapping_mul((0xe4) as i32)) as u32;
                eax = eax.wrapping_add(edx);
                push!(0x2000);
                ecx = eax.wrapping_add(0x2a8);
                push!(ecx);
                push!(0x428c0000);
                push!(0x1f);
                eax = eax.wrapping_add(0x288);
                push!(eax);
                pc = 0x328f1d; continue;
            }
            0x328f1d => {
                push!(0);
                push!(0);
                call!(0x7d560);
                pc = 0x328f26; continue;
            }
            0x328f26 => {
                esp = esp.wrapping_add(0x1c);
                pc = 0x328f29; continue;
            }
            0x328f29 => {
                push!(0xef);
                call!(0x716d0);
                pc = 0x328f33; continue;
            }
            0x328f33 => {
                esp = esp.wrapping_add(0x4);
                call!(0x783d0);
                pc = 0x328f3b; continue;
            }
            0x328f3b => {
                edx = m.read32(0x7844a8);
                eax = m.read32(edx.wrapping_add(0x11858));
                fa = m.read32(eax.wrapping_add(0x4));
                fb = ((((0x9i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x328fe9; continue; }
                pc = 0x328f51; continue;
            }
            0x328f51 => {
                push!(0);
                push!(0);
                push!(0x800);
                push!(ebx);
                push!(0xa2d);
                call!(0x3d7b0);
                pc = 0x328f65; continue;
            }
            0x328f65 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x328f6e; continue;
            }
            0x328f6e => {
                push!(0x2000);
                ecx = esi.wrapping_add(0x78);
                push!(ecx);
                push!(0x428c0000);
                push!(0x1f);
                push!(esi);
                push!(0);
                push!(0);
                call!(0x7d560);
                pc = 0x328f88; continue;
            }
            0x328f88 => {
                push!(0xf0);
                call!(0x716d0);
                pc = 0x328f92; continue;
            }
            0x328f92 => {
                esp = esp.wrapping_add(0x34);
                call!(0x783d0);
                pc = 0x328f9a; continue;
            }
            0x328f9a => {
                push!(0);
                push!(0);
                push!(0x800);
                push!(ebx);
                push!(0xa2e);
                call!(0x3d7b0);
                pc = 0x328fae; continue;
            }
            0x328fae => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x328fb7; continue;
            }
            0x328fb7 => {
                eax = (((((m.read16(esi.wrapping_add(0x74)) as i16)) as i32)) as u32);
                push!(0);
                edx = esi.wrapping_add(0x20);
                push!(edx);
                push!(0x43960000);
                push!(0x3f);
                eax = eax.wrapping_add(0x778d40);
                push!(eax);
                push!(0);
                push!(0);
                call!(0x7d560);
                pc = 0x328fd7; continue;
            }
            0x328fd7 => {
                push!(0xf1);
                call!(0x716d0);
                pc = 0x328fe1; continue;
            }
            0x328fe1 => {
                esp = esp.wrapping_add(0x34);
                call!(0x783d0);
                pc = 0x328fe9; continue;
            }
            0x328fe9 => {
                ecx = m.read32(0x7844a8);
                edx = m.read32(ecx.wrapping_add(0x11858));
                fa = m.read32(edx.wrapping_add(0x4));
                fb = ((((0x7i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3290dc; continue; }
                pc = 0x328fff; continue;
            }
            0x328fff => {
                push!(0);
                push!(0);
                push!(0x800);
                push!(ebx);
                push!(0xa2b);
                call!(0x3d7b0);
                pc = 0x329013; continue;
            }
            0x329013 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x32901c; continue;
            }
            0x32901c => {
                ecx = (((((m.read16(esi.wrapping_add(0x74)) as i16)) as i32)) as u32);
                push!(0);
                eax = esi.wrapping_add(0x20);
                push!(eax);
                push!(0x43960000);
                push!(0x3f);
                ecx = ecx.wrapping_add(0x778d40);
                push!(ecx);
                push!(0);
                push!(0);
                call!(0x7d560);
                pc = 0x32903d; continue;
            }
            0x32903d => {
                push!(0xf2);
                call!(0x716d0);
                pc = 0x329047; continue;
            }
            0x329047 => {
                esp = esp.wrapping_add(0x34);
                call!(0x783d0);
                pc = 0x32904f; continue;
            }
            0x32904f => {
                push!(0);
                push!(0);
                push!(0x800);
                push!(ebx);
                push!(0xbc8);
                call!(0x3d7b0);
                pc = 0x329063; continue;
            }
            0x329063 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x32906c; continue;
            }
            0x32906c => {
                push!(0);
                push!(0x800);
                call!(0x7d4a0);
                pc = 0x329078; continue;
            }
            0x329078 => {
                push!(0xf3);
                call!(0x716d0);
                pc = 0x329082; continue;
            }
            0x329082 => {
                esp = esp.wrapping_add(0x20);
                push!(0);
                push!(0xbc9);
                call!(0x3d7b0);
                pc = 0x329091; continue;
            }
            0x329091 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x32909a; continue;
            }
            0x32909a => {
                esp = esp.wrapping_add(0x8);
                push!(0x1);
                push!(0xbca);
                call!(0x3d7b0);
                pc = 0x3290a9; continue;
            }
            0x3290a9 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x3290b2; continue;
            }
            0x3290b2 => {
                eax = m.read32(0x7844a8);
                edx = m.read32(eax.wrapping_add(0x11858));
                ecx = (((((m.read16(edx.wrapping_add(0xc)) as i16)) as i32)) as u32);
                ecx = ((ecx as i32).wrapping_mul((0x8c) as i32)) as u32;
                edx = ecx.wrapping_add(eax).wrapping_add(0xdd90);
                push!(edx);
                call!(0x78590);
                pc = 0x3290d4; continue;
            }
            0x3290d4 => {
                esp = esp.wrapping_add(0xc);
                call!(0x783d0);
                pc = 0x3290dc; continue;
            }
            0x3290dc => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x11858));
                fa = m.read32(ecx.wrapping_add(0x4));
                fb = ((((0xdi32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3292c1; continue; }
                pc = 0x3290f1; continue;
            }
            0x3290f1 => {
                call!(0x315d10);
                pc = 0x3290f6; continue;
            }
            0x3290f6 => {
                ebp = eax;
                eax = m.read32(0x7844a8);
                edx = m.read32(eax.wrapping_add(0x11858));
                ecx = (((((m.read16(edx.wrapping_add(0xc)) as i16)) as i32)) as u32);
                ecx = ((ecx as i32).wrapping_mul((0x8c) as i32)) as u32;
                eax = ecx.wrapping_add(eax).wrapping_add(0xdd14);
                ebx = eax.wrapping_add(0x84);
                m.write32(esp.wrapping_add(0x24), eax);
                fa = m.read32(ebx);
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x32912a; continue; }
                pc = 0x329123; continue;
            }
            0x329123 => {
                edx = m.read32(ebp.wrapping_add(0x4));
                eax = m.read32(edx);
                m.write32(ebx, eax);
                pc = 0x32912a; continue;
            }
            0x32912a => {
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0xbcd);
                call!(0x3d7b0);
                pc = 0x32913c; continue;
            }
            0x32913c => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x329145; continue;
            }
            0x329145 => {
                push!(0x2);
                push!(0);
                push!(0);
                call!(0x78320);
                pc = 0x329150; continue;
            }
            0x329150 => {
                esp = esp.wrapping_add(0x20);
                call!(0x3201d0);
                pc = 0x329158; continue;
            }
            0x329158 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                xmm0 = Xmm::scalar(m.read32(0x49f820));
                if (fa != fb) { pc = 0x32916c; continue; }
                pc = 0x329164; continue;
            }
            0x329164 => {
                xmm0 = Xmm::scalar(m.read32(0x49f81c));
                pc = 0x32916c; continue;
            }
            0x32916c => {
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                ecx = m.read32(esp.wrapping_add(0x10));
                push!(0x320250);
                push!(0);
                push!(ecx);
                call!(0x71c60);
                pc = 0x329183; continue;
            }
            0x329183 => {
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(0x49f7f8)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(ecx);
                m.write32(esp, (fpu.st(0) as f32).to_bits()); fpu.pop();
                push!(0x4);
                push!(0xffffffffu32);
                push!(0);
                call!(0x792b0);
                pc = 0x329198; continue;
            }
            0x329198 => {
                esp = esp.wrapping_add(0x1c);
                call!(0x3201d0);
                pc = 0x3291a0; continue;
            }
            0x3291a0 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                xmm0 = Xmm::scalar(m.read32(0x49f820));
                if (fa != fb) { pc = 0x3291b4; continue; }
                pc = 0x3291ac; continue;
            }
            0x3291ac => {
                xmm0 = Xmm::scalar(m.read32(0x49f81c));
                pc = 0x3291b4; continue;
            }
            0x3291b4 => {
                push!(0x320460);
                push!(0x320660);
                m.write32(esp.wrapping_add(0x18), xmm0.lanes[0]);
                edx = m.read32(esp.wrapping_add(0x18));
                push!(edx);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0);
                call!(0x792b0);
                pc = 0x3291d6; continue;
            }
            0x3291d6 => {
                push!(0xff);
                call!(0x716d0);
                pc = 0x3291e0; continue;
            }
            0x3291e0 => {
                esp = esp.wrapping_add(0x20);
                call!(0x78380);
                pc = 0x3291e8; continue;
            }
            0x3291e8 => {
                call!(0x783d0);
                pc = 0x3291ed; continue;
            }
            0x3291ed => {
                push!(0);
                push!(0);
                push!(0x800);
                push!(0xffffffffu32);
                push!(0xbce);
                call!(0x3d7b0);
                pc = 0x329202; continue;
            }
            0x329202 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x32920b; continue;
            }
            0x32920b => {
                push!(0);
                push!(0x800);
                call!(0x7d4a0);
                pc = 0x329217; continue;
            }
            0x329217 => {
                push!(0xf4);
                call!(0x716d0);
                pc = 0x329221; continue;
            }
            0x329221 => {
                eax = m.read32(ebp.wrapping_add(0x8));
                esp = esp.wrapping_add(0x20);
                edi = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x32925e; continue; }
                pc = 0x32922d; continue;
            }
            0x32922d => {
                pc = 0x329230; continue;
            }
            0x329230 => {
                eax = m.read32(ebp.wrapping_add(0x4));
                eax = m.read32(eax.wrapping_add(edi.wrapping_mul(0x4)));
                push!(eax);
                eax = ((eax as i32).wrapping_mul((0xc4) as i32)) as u32;
                ecx = (((((m.read16(eax.wrapping_add(0x524f58)) as i16)) as i32)) as u32);
                push!(ecx);
                call!(0x3d7b0);
                pc = 0x32924a; continue;
            }
            0x32924a => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x329253; continue;
            }
            0x329253 => {
                eax = m.read32(ebp.wrapping_add(0x8));
                esp = esp.wrapping_add(0x8);
                edi = edi.wrapping_add(1);
                fa = edi;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x329230; continue; }
                pc = 0x32925e; continue;
            }
            0x32925e => {
                push!(ebx);
                call!(0x78590);
                pc = 0x329264; continue;
            }
            0x329264 => {
                push!(0x3f800000);
                call!(0x795b0);
                pc = 0x32926e; continue;
            }
            0x32926e => {
                esp = esp.wrapping_add(0x8);
                push!(0);
                push!(0);
                push!(0);
                push!(0x800);
                push!(0xffffffffu32);
                push!(0xbd0);
                call!(0x3d7b0);
                pc = 0x329288; continue;
            }
            0x329288 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x329291; continue;
            }
            0x329291 => {
                push!(0);
                push!(0);
                call!(0x7d4f0);
                pc = 0x32929a; continue;
            }
            0x32929a => {
                push!(0xf5);
                call!(0x716d0);
                pc = 0x3292a4; continue;
            }
            0x3292a4 => {
                edx = m.read32(esp.wrapping_add(0x48));
                edx = edx.wrapping_add(0x24);
                push!(edx);
                call!(0x78560);
                pc = 0x3292b1; continue;
            }
            0x3292b1 => {
                esp = esp.wrapping_add(0x28);
                call!(0x783d0);
                pc = 0x3292b9; continue;
            }
            0x3292b9 => {
                ebp = m.read32(esp.wrapping_add(0x18));
                edi = m.read32(esp.wrapping_add(0x14));
                pc = 0x3292c1; continue;
            }
            0x3292c1 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x11858));
                fa = m.read32(ecx.wrapping_add(0x4));
                fb = ((((0xai32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x329462; continue; }
                pc = 0x3292d6; continue;
            }
            0x3292d6 => {
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0xbc4);
                call!(0x3d7b0);
                pc = 0x3292e8; continue;
            }
            0x3292e8 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x3292f1; continue;
            }
            0x3292f1 => {
                push!(0x2);
                push!(0);
                push!(0);
                call!(0x78320);
                pc = 0x3292fc; continue;
            }
            0x3292fc => {
                esp = esp.wrapping_add(0x20);
                call!(0x3201d0);
                pc = 0x329304; continue;
            }
            0x329304 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                xmm0 = Xmm::scalar(m.read32(0x49f820));
                if (fa != fb) { pc = 0x329318; continue; }
                pc = 0x329310; continue;
            }
            0x329310 => {
                xmm0 = Xmm::scalar(m.read32(0x49f81c));
                pc = 0x329318; continue;
            }
            0x329318 => {
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                edx = m.read32(esp.wrapping_add(0x10));
                push!(0x319e60);
                push!(0);
                push!(edx);
                call!(0x71c60);
                pc = 0x32932f; continue;
            }
            0x32932f => {
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(0x49f7f8)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(ecx);
                m.write32(esp, (fpu.st(0) as f32).to_bits()); fpu.pop();
                push!(0x4);
                push!(0xbc5);
                push!(0);
                call!(0x792b0);
                pc = 0x329347; continue;
            }
            0x329347 => {
                esp = esp.wrapping_add(0x1c);
                call!(0x3201d0);
                pc = 0x32934f; continue;
            }
            0x32934f => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                xmm0 = Xmm::scalar(m.read32(0x49f820));
                if (fa != fb) { pc = 0x329363; continue; }
                pc = 0x32935b; continue;
            }
            0x32935b => {
                xmm0 = Xmm::scalar(m.read32(0x49f81c));
                pc = 0x329363; continue;
            }
            0x329363 => {
                push!(0x31ad90);
                push!(0x320030);
                m.write32(esp.wrapping_add(0x18), xmm0.lanes[0]);
                eax = m.read32(esp.wrapping_add(0x18));
                push!(eax);
                push!(0);
                push!(0);
                push!(0x40);
                push!(0);
                call!(0x792b0);
                pc = 0x329385; continue;
            }
            0x329385 => {
                push!(0x100);
                call!(0x716d0);
                pc = 0x32938f; continue;
            }
            0x32938f => {
                esp = esp.wrapping_add(0x20);
                call!(0x78380);
                pc = 0x329397; continue;
            }
            0x329397 => {
                call!(0x783d0);
                pc = 0x32939c; continue;
            }
            0x32939c => {
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0xbc6);
                call!(0x3d7b0);
                pc = 0x3293ae; continue;
            }
            0x3293ae => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x3293b7; continue;
            }
            0x3293b7 => {
                push!(0x2);
                push!(0);
                push!(0);
                call!(0x78320);
                pc = 0x3293c2; continue;
            }
            0x3293c2 => {
                esp = esp.wrapping_add(0x20);
                call!(0x3201d0);
                pc = 0x3293ca; continue;
            }
            0x3293ca => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                xmm0 = Xmm::scalar(m.read32(0x49f820));
                if (fa != fb) { pc = 0x3293de; continue; }
                pc = 0x3293d6; continue;
            }
            0x3293d6 => {
                xmm0 = Xmm::scalar(m.read32(0x49f81c));
                pc = 0x3293de; continue;
            }
            0x3293de => {
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                ecx = m.read32(esp.wrapping_add(0x10));
                push!(0x319e60);
                push!(0);
                push!(ecx);
                call!(0x71c60);
                pc = 0x3293f5; continue;
            }
            0x3293f5 => {
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(0x49f7f8)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(ecx);
                m.write32(esp, (fpu.st(0) as f32).to_bits()); fpu.pop();
                push!(0x4);
                push!(0xbc7);
                push!(0);
                call!(0x792b0);
                pc = 0x32940d; continue;
            }
            0x32940d => {
                esp = esp.wrapping_add(0x1c);
                call!(0x3201d0);
                pc = 0x329415; continue;
            }
            0x329415 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                xmm0 = Xmm::scalar(m.read32(0x49f820));
                if (fa != fb) { pc = 0x329429; continue; }
                pc = 0x329421; continue;
            }
            0x329421 => {
                xmm0 = Xmm::scalar(m.read32(0x49f81c));
                pc = 0x329429; continue;
            }
            0x329429 => {
                push!(0x31ad90);
                push!(0x31a600);
                m.write32(esp.wrapping_add(0x18), xmm0.lanes[0]);
                edx = m.read32(esp.wrapping_add(0x18));
                push!(edx);
                push!(0);
                push!(0);
                push!(0x40);
                push!(0);
                call!(0x792b0);
                pc = 0x32944b; continue;
            }
            0x32944b => {
                push!(0x101);
                call!(0x716d0);
                pc = 0x329455; continue;
            }
            0x329455 => {
                esp = esp.wrapping_add(0x20);
                call!(0x78380);
                pc = 0x32945d; continue;
            }
            0x32945d => {
                call!(0x783d0);
                pc = 0x329462; continue;
            }
            0x329462 => {
                eax = (eax & 0xffff_0000) | ((((m.read16(edi.wrapping_add(0xc))) as u32)) & 0xffff);
                fa = ((((((eax) as u16))) as u32) & 0xffffu32);
                fb = ((((0xffffi32)) as u32) & 0xffffu32);
                fas = (((((fa)) as i16)) as i32);
                fbs = (((((fb)) as i16)) as i32);
                if (fas <= fbs) { pc = 0x3294f6; continue; }
                pc = 0x329470; continue;
            }
            0x329470 => {
                ecx = m.read32(0x7844a8);
                eax = ((((eax) as u16)) as u16 as i16 as i32 as u32);
                eax = ((eax as i32).wrapping_mul((0x8c) as i32)) as u32;
                edx = eax.wrapping_add(ecx).wrapping_add(0xdd14);
                push!(edx);
                call!(0x2fe450);
                pc = 0x32948c; continue;
            }
            0x32948c => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3294f6; continue; }
                pc = 0x329493; continue;
            }
            0x329493 => {
                eax = m.read32(esp.wrapping_add(0x14));
                ecx = (((((m.read16(eax.wrapping_add(0xc)) as i16)) as i32)) as u32);
                push!(ecx);
                call!(0x3031b0);
                pc = 0x3294a1; continue;
            }
            0x3294a1 => {
                edi = eax;
                esp = esp.wrapping_add(0x4);
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3294b9; continue; }
                pc = 0x3294aa; continue;
            }
            0x3294aa => {
                push!(0xb84);
                call!(0x3d7b0);
                pc = 0x3294b4; continue;
            }
            0x3294b4 => {
                esp = esp.wrapping_add(0x4);
                edi = eax;
                pc = 0x3294b9; continue;
            }
            0x3294b9 => {
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0x9e7);
                call!(0x3d7b0);
                pc = 0x3294cb; continue;
            }
            0x3294cb => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x3294d4; continue;
            }
            0x3294d4 => {
                push!(0);
                push!(0);
                push!(0x40000000);
                push!(0x804);
                push!(0xffffffffu32);
                push!(edi);
                call!(0x79d60);
                pc = 0x3294ea; continue;
            }
            0x3294ea => {
                esp = esp.wrapping_add(0x2c);
                call!(0x783d0);
                pc = 0x3294f2; continue;
            }
            0x3294f2 => {
                ebp = m.read32(esp.wrapping_add(0x18));
                pc = 0x3294f6; continue;
            }
            0x3294f6 => {
                edx = m.read32(0x7844a8);
                eax = m.read32(edx.wrapping_add(0x11858));
                eax = m.read32(eax.wrapping_add(0x4));
                fa = eax;
                fb = ((((0x7i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x329518; continue; }
                pc = 0x32950a; continue;
            }
            0x32950a => {
                fa = eax;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x329518; continue; }
                pc = 0x32950f; continue;
            }
            0x32950f => {
                fa = eax;
                fb = ((((0x5i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3295a6; continue; }
                pc = 0x329518; continue;
            }
            0x329518 => {
                push!(0);
                push!(0);
                push!(0x800);
                push!(0xffffffffu32);
                push!(0xb05);
                call!(0x3d7b0);
                pc = 0x32952d; continue;
            }
            0x32952d => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x329536; continue;
            }
            0x329536 => {
                push!(0);
                push!(0x800);
                call!(0x7d4a0);
                pc = 0x329542; continue;
            }
            0x329542 => {
                push!(0xf6);
                call!(0x716d0);
                pc = 0x32954c; continue;
            }
            0x32954c => {
                esp = esp.wrapping_add(0x20);
                push!(0x1);
                push!(0xc06);
                call!(0x3d7b0);
                pc = 0x32955b; continue;
            }
            0x32955b => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x329564; continue;
            }
            0x329564 => {
                esp = esp.wrapping_add(0x8);
                push!(0);
                push!(0xc07);
                call!(0x3d7b0);
                pc = 0x329573; continue;
            }
            0x329573 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x32957c; continue;
            }
            0x32957c => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x11858));
                edx = (((((m.read16(ecx.wrapping_add(0xc)) as i16)) as i32)) as u32);
                edx = ((edx as i32).wrapping_mul((0x8c) as i32)) as u32;
                eax = edx.wrapping_add(eax).wrapping_add(0xdd9c);
                push!(eax);
                call!(0x78590);
                pc = 0x32959e; continue;
            }
            0x32959e => {
                esp = esp.wrapping_add(0xc);
                call!(0x783d0);
                pc = 0x3295a6; continue;
            }
            0x3295a6 => {
                ecx = m.read32(0x7844a8);
                edx = m.read32(ecx.wrapping_add(0x11858));
                fa = m.read32(edx.wrapping_add(0x4));
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x32967f; continue; }
                pc = 0x3295bc; continue;
            }
            0x3295bc => {
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0xbd6);
                call!(0x3d7b0);
                pc = 0x3295ce; continue;
            }
            0x3295ce => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x3295d7; continue;
            }
            0x3295d7 => {
                push!(0x2);
                push!(0);
                push!(0);
                call!(0x78320);
                pc = 0x3295e2; continue;
            }
            0x3295e2 => {
                esp = esp.wrapping_add(0x20);
                call!(0x3201d0);
                pc = 0x3295ea; continue;
            }
            0x3295ea => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                xmm0 = Xmm::scalar(m.read32(0x49f820));
                if (fa != fb) { pc = 0x3295fe; continue; }
                pc = 0x3295f6; continue;
            }
            0x3295f6 => {
                xmm0 = Xmm::scalar(m.read32(0x49f81c));
                pc = 0x3295fe; continue;
            }
            0x3295fe => {
                m.write32(esp.wrapping_add(0x18), xmm0.lanes[0]);
                eax = m.read32(esp.wrapping_add(0x18));
                push!(0x320250);
                push!(0);
                push!(eax);
                call!(0x71c60);
                pc = 0x329615; continue;
            }
            0x329615 => {
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(0x49f7f8)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(ecx);
                m.write32(esp, (fpu.st(0) as f32).to_bits()); fpu.pop();
                push!(0x4);
                push!(0xffffffffu32);
                push!(0);
                call!(0x792b0);
                pc = 0x32962a; continue;
            }
            0x32962a => {
                esp = esp.wrapping_add(0x1c);
                call!(0x3201d0);
                pc = 0x329632; continue;
            }
            0x329632 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                xmm0 = Xmm::scalar(m.read32(0x49f820));
                if (fa != fb) { pc = 0x329646; continue; }
                pc = 0x32963e; continue;
            }
            0x32963e => {
                xmm0 = Xmm::scalar(m.read32(0x49f81c));
                pc = 0x329646; continue;
            }
            0x329646 => {
                push!(0x320460);
                push!(0x3207c0);
                m.write32(esp.wrapping_add(0x20), xmm0.lanes[0]);
                ecx = m.read32(esp.wrapping_add(0x20));
                push!(ecx);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0);
                call!(0x792b0);
                pc = 0x329668; continue;
            }
            0x329668 => {
                push!(0x102);
                call!(0x716d0);
                pc = 0x329672; continue;
            }
            0x329672 => {
                esp = esp.wrapping_add(0x20);
                call!(0x78380);
                pc = 0x32967a; continue;
            }
            0x32967a => {
                call!(0x783d0);
                pc = 0x32967f; continue;
            }
            0x32967f => {
                edx = m.read32(0x7844a8);
                eax = m.read32(edx.wrapping_add(0x11858));
                fa = m.read32(eax.wrapping_add(0x4));
                fb = ((((0xei32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x329787; continue; }
                pc = 0x329695; continue;
            }
            0x329695 => {
                push!(0);
                push!(0);
                push!(0x800);
                push!(0xffffffffu32);
                push!(0xbd8);
                call!(0x3d7b0);
                pc = 0x3296aa; continue;
            }
            0x3296aa => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x3296b3; continue;
            }
            0x3296b3 => {
                edx = (((((m.read16(esi.wrapping_add(0x74)) as i16)) as i32)) as u32);
                push!(0);
                ecx = esi.wrapping_add(0x20);
                push!(ecx);
                push!(0x43960000);
                push!(0x3f);
                edx = edx.wrapping_add(0x778d40);
                push!(edx);
                push!(0);
                push!(0);
                call!(0x7d560);
                pc = 0x3296d4; continue;
            }
            0x3296d4 => {
                push!(0xf7);
                call!(0x716d0);
                pc = 0x3296de; continue;
            }
            0x3296de => {
                esp = esp.wrapping_add(0x34);
                call!(0x783d0);
                pc = 0x3296e6; continue;
            }
            0x3296e6 => {
                push!(0);
                push!(0);
                push!(0x800);
                push!(0xffffffffu32);
                push!(0xbd9);
                call!(0x3d7b0);
                pc = 0x3296fb; continue;
            }
            0x3296fb => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x329704; continue;
            }
            0x329704 => {
                ecx = (((((m.read16(esi.wrapping_add(0x74)) as i16)) as i32)) as u32);
                push!(0);
                eax = esi.wrapping_add(0x24);
                push!(eax);
                push!(0x43960000);
                push!(0x3f);
                ecx = ecx.wrapping_add(0x778d80);
                push!(ecx);
                push!(0);
                push!(0);
                call!(0x7d560);
                pc = 0x329725; continue;
            }
            0x329725 => {
                push!(0xf8);
                call!(0x716d0);
                pc = 0x32972f; continue;
            }
            0x32972f => {
                esp = esp.wrapping_add(0x34);
                call!(0x783d0);
                pc = 0x329737; continue;
            }
            0x329737 => {
                push!(0);
                push!(0);
                push!(0x800);
                push!(0xffffffffu32);
                push!(0xbda);
                call!(0x3d7b0);
                pc = 0x32974c; continue;
            }
            0x32974c => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x329755; continue;
            }
            0x329755 => {
                eax = (((((m.read16(esi.wrapping_add(0x74)) as i16)) as i32)) as u32);
                push!(0);
                edx = esi.wrapping_add(0x28);
                push!(edx);
                push!(0x43960000);
                push!(0x3f);
                eax = eax.wrapping_add(0x778dc0);
                push!(eax);
                push!(0);
                push!(0);
                call!(0x7d560);
                pc = 0x329775; continue;
            }
            0x329775 => {
                push!(0xf9);
                call!(0x716d0);
                pc = 0x32977f; continue;
            }
            0x32977f => {
                esp = esp.wrapping_add(0x34);
                call!(0x783d0);
                pc = 0x329787; continue;
            }
            0x329787 => {
                ecx = m.read32(esp.wrapping_add(0x1c));
                push!(ecx);
                call!(0x151090);
                pc = 0x329791; continue;
            }
            0x329791 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x3297aa; continue; }
                pc = 0x329799; continue;
            }
            0x329799 => {
                edx = m.read32(0x7844a8);
                eax = m.read32(edx.wrapping_add(0x11858));
                call!(0x325290);
                pc = 0x3297aa; continue;
            }
            0x3297aa => {
                fa = ((((m.read8(ebp.wrapping_add(0x20)))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x3299ec; continue; }
                pc = 0x3297b4; continue;
            }
            0x3297b4 => {
                edx = m.read32(0x7844a8);
                edi = m.read32(edx.wrapping_add(0x11858));
                ebx = m.read32(edi);
                fa = ebx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3297cc; continue; }
                pc = 0x3297c7; continue;
            }
            0x3297c7 => {
                fa = ebx;
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3297d6; continue; }
                pc = 0x3297cc; continue;
            }
            0x3297cc => {
                fa = m.read32(edi.wrapping_add(0x4));
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3299ec; continue; }
                pc = 0x3297d6; continue;
            }
            0x3297d6 => {
                eax = m.read32(edx.wrapping_add(0x11858));
                esi = m.read32(eax.wrapping_add(0x8));
                ecx = m.read32(eax.wrapping_add(0x4));
                eax = m.read32(eax);
                fa = eax;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3297ee; continue; }
                pc = 0x3297e9; continue;
            }
            0x3297e9 => {
                fa = ecx;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x32981d; continue; }
                pc = 0x3297ee; continue;
            }
            0x3297ee => {
                fa = ecx;
                fb = ((((0x5i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x329801; continue; }
                pc = 0x3297f3; continue;
            }
            0x3297f3 => {
                fa = ecx;
                fb = ((((0x6i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x329801; continue; }
                pc = 0x3297f8; continue;
            }
            0x3297f8 => {
                fa = ecx;
                fb = ((((0xdi32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x329b57; continue; }
                pc = 0x329801; continue;
            }
            0x329801 => {
                fa = eax;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x32981d; continue; }
                pc = 0x329806; continue;
            }
            0x329806 => {
                fa = eax;
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x329b57; continue; }
                pc = 0x32980f; continue;
            }
            0x32980f => {
                fa = esi;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x32981d; continue; }
                pc = 0x329814; continue;
            }
            0x329814 => {
                fa = esi;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x329b57; continue; }
                pc = 0x32981d; continue;
            }
            0x32981d => {
                fa = ebx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x32998b; continue; }
                pc = 0x329826; continue;
            }
            0x329826 => {
                eax = m.read32(edi.wrapping_add(0x4));
                fa = eax;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x32988d; continue; }
                pc = 0x32982e; continue;
            }
            0x32982e => {
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0x9df);
                call!(0x3d7b0);
                pc = 0x329840; continue;
            }
            0x329840 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x329849; continue;
            }
            0x329849 => {
                eax = m.read32(ebp.wrapping_add(0xc));
                esp = esp.wrapping_add(0x14);
                push!(0x60);
                push!(eax);
                call!(0x3d7b0);
                pc = 0x329857; continue;
            }
            0x329857 => {
                ecx = m.read32(0x7844a8);
                edx = m.read32(ecx.wrapping_add(0x11858));
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(ebx);
                push!(0x258);
                push!(ebx);
                edx = edx.wrapping_add(0x10);
                push!(edx);
                push!(0);
                push!(0);
                call!(0x7d5e0);
                pc = 0x32987b; continue;
            }
            0x32987b => {
                push!(0xfb);
                call!(0x716d0);
                pc = 0x329885; continue;
            }
            0x329885 => {
                esp = esp.wrapping_add(0x24);
                pc = 0x329b52; continue;
            }
            0x32988d => {
                fa = eax;
                fb = ((((0xdi32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x32992a; continue; }
                pc = 0x329896; continue;
            }
            0x329896 => {
                eax = m.read32(edx.wrapping_add(0x11858));
                ecx = (((((m.read16(eax.wrapping_add(0xc)) as i16)) as i32)) as u32);
                esi = m.read32(eax.wrapping_add(0x8));
                ecx = ((ecx as i32).wrapping_mul((0x8c) as i32)) as u32;
                eax = m.read32(ecx.wrapping_add(edx).wrapping_add(0xdd98));
                edi = ecx.wrapping_add(edx).wrapping_add(0xdd14);
                edx = 0;
                fa = esi;
                fb = ((((0x9i32)) as u32) & 0xffffffffu32);
                edx = (edx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                push!(eax);
                ebp = edx;
                call!(0x308340);
                pc = 0x3298c7; continue;
            }
            0x3298c7 => {
                esp = esp.wrapping_add(0x4);
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                esi = eax;
                if (fa == fb) { pc = 0x3298d3; continue; }
                pc = 0x3298d0; continue;
            }
            0x3298d0 => {
                m.write32(edi.wrapping_add(0x20), esi);
                pc = 0x3298d3; continue;
            }
            0x3298d3 => {
                eax = m.read32(edi.wrapping_add(0x20));
                edi = edi.wrapping_add(0x20);
                fa = eax;
                fb = esi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x3298df; continue; }
                pc = 0x3298dd; continue;
            }
            0x3298dd => {
                m.write32(edi, esi);
                pc = 0x3298df; continue;
            }
            0x3298df => {
                push!(0);
                push!(0);
                push!(ebp);
                push!(0xffffffffu32);
                push!(0x9df);
                call!(0x3d7b0);
                pc = 0x3298f0; continue;
            }
            0x3298f0 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x3298f9; continue;
            }
            0x3298f9 => {
                esp = esp.wrapping_add(0x14);
                push!(0);
                push!(0);
                call!(0x3d7b0);
                pc = 0x329905; continue;
            }
            0x329905 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x1);
                push!(esi);
                push!(0);
                push!(edi);
                push!(0);
                push!(0);
                call!(0x7d5e0);
                pc = 0x329918; continue;
            }
            0x329918 => {
                push!(0xfc);
                call!(0x716d0);
                pc = 0x329922; continue;
            }
            0x329922 => {
                esp = esp.wrapping_add(0x24);
                pc = 0x329b52; continue;
            }
            0x32992a => {
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0x9df);
                call!(0x3d7b0);
                pc = 0x32993c; continue;
            }
            0x32993c => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x329945; continue;
            }
            0x329945 => {
                ecx = m.read32(ebp.wrapping_add(0xc));
                esp = esp.wrapping_add(0x14);
                push!(0);
                push!(ecx);
                call!(0x3d7b0);
                pc = 0x329953; continue;
            }
            0x329953 => {
                edx = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x4);
                push!(eax);
                eax = m.read32(edx.wrapping_add(0x11858));
                push!(0x1);
                push!(0xf4240);
                push!(0);
                eax = eax.wrapping_add(0x10);
                push!(eax);
                push!(0);
                push!(0);
                call!(0x7d5e0);
                pc = 0x329979; continue;
            }
            0x329979 => {
                push!(0xfd);
                call!(0x716d0);
                pc = 0x329983; continue;
            }
            0x329983 => {
                esp = esp.wrapping_add(0x24);
                pc = 0x329b52; continue;
            }
            0x32998b => {
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0xbbf);
                call!(0x3d7b0);
                pc = 0x32999d; continue;
            }
            0x32999d => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x3299a6; continue;
            }
            0x3299a6 => {
                ecx = m.read32(ebp.wrapping_add(0xc));
                esp = esp.wrapping_add(0x14);
                push!(0);
                push!(ecx);
                call!(0x3d7b0);
                pc = 0x3299b4; continue;
            }
            0x3299b4 => {
                edx = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x4);
                push!(eax);
                eax = m.read32(edx.wrapping_add(0x11858));
                push!(0x1);
                push!(0x2710);
                push!(0x1);
                eax = eax.wrapping_add(0x10);
                push!(eax);
                push!(0);
                push!(0);
                call!(0x7d5e0);
                pc = 0x3299da; continue;
            }
            0x3299da => {
                push!(0xfe);
                call!(0x716d0);
                pc = 0x3299e4; continue;
            }
            0x3299e4 => {
                esp = esp.wrapping_add(0x24);
                pc = 0x329b52; continue;
            }
            0x3299ec => {
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0xb83);
                call!(0x3d7b0);
                pc = 0x3299fe; continue;
            }
            0x3299fe => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79460);
                pc = 0x329a07; continue;
            }
            0x329a07 => {
                push!(0x2);
                push!(0);
                push!(0);
                call!(0x78320);
                pc = 0x329a12; continue;
            }
            0x329a12 => {
                esp = esp.wrapping_add(0x20);
                call!(0x3201d0);
                pc = 0x329a1a; continue;
            }
            0x329a1a => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                xmm0 = Xmm::scalar(m.read32(0x49f820));
                if (fa != fb) { pc = 0x329a2e; continue; }
                pc = 0x329a26; continue;
            }
            0x329a26 => {
                xmm0 = Xmm::scalar(m.read32(0x49f81c));
                pc = 0x329a2e; continue;
            }
            0x329a2e => {
                push!(0x320250);
                m.write32(esp.wrapping_add(0x20), xmm0.lanes[0]);
                ecx = m.read32(esp.wrapping_add(0x20));
                push!(0);
                push!(ecx);
                call!(0x71c60);
                pc = 0x329a45; continue;
            }
            0x329a45 => {
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(0x49f7f8)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(ecx);
                m.write32(esp, (fpu.st(0) as f32).to_bits()); fpu.pop();
                push!(0x4);
                push!(0xffffffffu32);
                push!(0);
                call!(0x792b0);
                pc = 0x329a5a; continue;
            }
            0x329a5a => {
                edx = m.read32(0x7844a8);
                eax = m.read32(edx.wrapping_add(0x11858));
                ecx = m.read32(eax);
                esp = esp.wrapping_add(0x1c);
                fa = ecx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x329a75; continue; }
                pc = 0x329a70; continue;
            }
            0x329a70 => {
                fa = ecx;
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x329abd; continue; }
                pc = 0x329a75; continue;
            }
            0x329a75 => {
                fa = m.read32(eax.wrapping_add(0x4));
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x329abd; continue; }
                pc = 0x329a7b; continue;
            }
            0x329a7b => {
                call!(0x3201d0);
                pc = 0x329a80; continue;
            }
            0x329a80 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                xmm0 = Xmm::scalar(m.read32(0x49f820));
                if (fa != fb) { pc = 0x329a94; continue; }
                pc = 0x329a8c; continue;
            }
            0x329a8c => {
                xmm0 = Xmm::scalar(m.read32(0x49f81c));
                pc = 0x329a94; continue;
            }
            0x329a94 => {
                push!(0x320580);
                push!(0);
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                eax = m.read32(esp.wrapping_add(0x24));
                push!(eax);
                push!(0);
                push!(0x4);
                push!(0xffffffffu32);
                push!(0);
                call!(0x792b0);
                pc = 0x329ab3; continue;
            }
            0x329ab3 => {
                push!(0x103);
                pc = 0x329b45; continue;
            }
            0x329abd => {
                fa = m.read32(eax.wrapping_add(0x4));
                fb = ((((0xbi32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x329b05; continue; }
                pc = 0x329ac3; continue;
            }
            0x329ac3 => {
                call!(0x3201d0);
                pc = 0x329ac8; continue;
            }
            0x329ac8 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                xmm0 = Xmm::scalar(m.read32(0x49f820));
                if (fa != fb) { pc = 0x329adc; continue; }
                pc = 0x329ad4; continue;
            }
            0x329ad4 => {
                xmm0 = Xmm::scalar(m.read32(0x49f81c));
                pc = 0x329adc; continue;
            }
            0x329adc => {
                push!(0x320460);
                push!(0x320720);
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                ecx = m.read32(esp.wrapping_add(0x24));
                push!(ecx);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0);
                call!(0x792b0);
                pc = 0x329afe; continue;
            }
            0x329afe => {
                push!(0x104);
                pc = 0x329b45; continue;
            }
            0x329b05 => {
                call!(0x3201d0);
                pc = 0x329b0a; continue;
            }
            0x329b0a => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                xmm0 = Xmm::scalar(m.read32(0x49f820));
                if (fa != fb) { pc = 0x329b1e; continue; }
                pc = 0x329b16; continue;
            }
            0x329b16 => {
                xmm0 = Xmm::scalar(m.read32(0x49f81c));
                pc = 0x329b1e; continue;
            }
            0x329b1e => {
                push!(0x320460);
                push!(0x320660);
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                edx = m.read32(esp.wrapping_add(0x24));
                push!(edx);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0);
                call!(0x792b0);
                pc = 0x329b40; continue;
            }
            0x329b40 => {
                push!(0x105);
                pc = 0x329b45; continue;
            }
            0x329b45 => {
                call!(0x716d0);
                pc = 0x329b4a; continue;
            }
            0x329b4a => {
                esp = esp.wrapping_add(0x20);
                call!(0x78380);
                pc = 0x329b52; continue;
            }
            0x329b52 => {
                call!(0x783d0);
                pc = 0x329b57; continue;
            }
            0x329b57 => {
                push!(0x40000000);
                call!(0x784a0);
                pc = 0x329b61; continue;
            }
            0x329b61 => {
                eax = m.read32(0x778d3c);
                esp = esp.wrapping_add(0x4);
                push!(0);
                push!(0x328bf0);
                push!(0x40000000);
                cf = (((((eax) != 0))) as i32);
                eax = (((((eax) as i32).wrapping_neg())) as u32);
                push!(0);
                eax = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                eax = eax.wrapping_add(0xb5e);
                push!(0);
                push!(eax);
                call!(0x3d7b0);
                pc = 0x329b88; continue;
            }
            0x329b88 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x329b91; continue;
            }
            0x329b91 => {
                call!(0x781a0);
                pc = 0x329b96; continue;
            }
            0x329b96 => {
                call!(0x22b90);
                pc = 0x329b9b; continue;
            }
            0x329b9b => {
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
                pc = 0x329be6; continue;
            }
            0x329be6 => {
                esp = esp.wrapping_add(0x8);
                call!(0x75e70);
                pc = 0x329bee; continue;
            }
            0x329bee => {
                esi = m.read32(esp.wrapping_add(0x30));
                m.write32(esi.wrapping_add(0x218), (m.read32(esi.wrapping_add(0x218)) | 0x4));
                edx = m.read32(0x7844a8);
                eax = m.read32(edx.wrapping_add(0x11858));
                eax = (eax & 0xffff_0000) | ((((m.read16(eax.wrapping_add(0xc))) as u32)) & 0xffff);
                fa = ((((((eax) as u16))) as u32) & 0xffffu32);
                fb = ((((0xffffi32)) as u32) & 0xffffu32);
                fas = (((((fa)) as i16)) as i32);
                fbs = (((((fb)) as i16)) as i32);
                if (fas <= fbs) { pc = 0x329cf0; continue; }
                pc = 0x329c13; continue;
            }
            0x329c13 => {
                ecx = ((((eax) as u16)) as u16 as i16 as i32 as u32);
                push!(ecx);
                call!(0x2fe520);
                pc = 0x329c1c; continue;
            }
            0x329c1c => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x329cf0; continue; }
                pc = 0x329c27; continue;
            }
            0x329c27 => {
                edx = m.read32(0x7844a8);
                eax = m.read32(edx.wrapping_add(0x11858));
                ecx = (((((m.read16(eax.wrapping_add(0xc)) as i16)) as i32)) as u32);
                push!(ecx);
                edx = esp.wrapping_add(0x20);
                push!(edx);
                call!(0x2fe610);
                pc = 0x329c42; continue;
            }
            0x329c42 => {
                ecx = m.read32(eax);
                edi = (edi & 0xffff_0000) | ((((m.read16(eax.wrapping_add(0x4))) as u32)) & 0xffff);
                eax = m.read32(esi.wrapping_add(0x218));
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x50)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x4785c8))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475cd4))).to_bits();
                eax = (eax | 0x9);
                m.write32(esp.wrapping_add(0x2c), ecx);
                m.write32(esp.wrapping_add(0x38), xmm0.lanes[0]);
                m.write32(esi.wrapping_add(0x218), eax);
                call!(0x2f7a20);
                pc = 0x329c7b; continue;
            }
            0x329c7b => {
                xmm0 = Xmm::scalar(m.read32(0x7ba188));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x478900))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x7ba184))).to_bits();
                eax = (((((m.read16(esp.wrapping_add(0x2e)) as i16)) as i32)) as u32);
                ecx = (((((m.read16(esp.wrapping_add(0x2c)) as i16)) as i32)) as u32);
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x7ba038));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x38)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x7ba180))).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x475cd4));
                edx = ((((edi) as u16)) as u16 as i16 as i32 as u32);
                push!(edx);
                esp = esp.wrapping_sub(0x8);
                xmm1.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x4), xmm1.lanes[0]);
                xmm1.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp, xmm1.lanes[0]);
                call!(0x2f7af0);
                pc = 0x329ced; continue;
            }
            0x329ced => {
                esp = esp.wrapping_add(0x14);
                pc = 0x329cf0; continue;
            }
            0x329cf0 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x1c);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00329DE0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f329de0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x329de0;
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
        { crate::pose::cover(0x329de0, previous, pc); previous = pc; }
        match pc {
            0x329de0 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                push!(edi);
                edi = m.read32(esi.wrapping_add(0x4));
                eax = 0;
                fa = edi;
                fb = ((((0x52e8b0i32)) as u32) & 0xffffffffu32);
                eax = (eax & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                push!(0x3254d0);
                push!(0);
                push!(0x42480000);
                push!(0xb61);
                eax = eax.wrapping_sub(1);
                eax = (eax & 0x10);
                edi = eax;
                call!(0x3d7b0);
                pc = 0x329e10; continue;
            }
            0x329e10 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                edi = (edi | 0x8400);
                push!(edi);
                call!(0x75b70);
                pc = 0x329e20; continue;
            }
            0x329e20 => {
                push!(0x1);
                push!(0x1);
                push!(0);
                push!(0x1);
                call!(0x71b10);
                pc = 0x329e2d; continue;
            }
            0x329e2d => {
                push!(0x20000);
                call!(0x71c50);
                pc = 0x329e37; continue;
            }
            0x329e37 => {
                esp = esp.wrapping_add(0x28);
                push!(0);
                push!(0);
                push!(0);
                call!(0x22b90);
                pc = 0x329e45; continue;
            }
            0x329e45 => {
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
                pc = 0x329e93; continue;
            }
            0x329e93 => {
                eax = m.read32(esi.wrapping_add(0x4));
                esp = esp.wrapping_add(0x10);
                fa = eax;
                fb = ((((0x52e8b0i32)) as u32) & 0xffffffffu32);
                push!(0);
                push!(0);
                push!(0x800);
                push!(0xffffffffu32);
                if (fa != fb) { pc = 0x329fd3; continue; }
                pc = 0x329eaf; continue;
            }
            0x329eaf => {
                push!(0xb37);
                call!(0x3d7b0);
                pc = 0x329eb9; continue;
            }
            0x329eb9 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x329ec2; continue;
            }
            0x329ec2 => {
                ecx = m.read32(0x762f98);
                push!(0);
                push!(0);
                push!(0x40000000);
                push!(0x804);
                ecx = ecx.wrapping_add(0x20);
                push!(0xffffffffu32);
                push!(ecx);
                call!(0x79d60);
                pc = 0x329ee1; continue;
            }
            0x329ee1 => {
                esp = esp.wrapping_add(0x2c);
                call!(0x783d0);
                pc = 0x329ee9; continue;
            }
            0x329ee9 => {
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0xb39);
                call!(0x3d7b0);
                pc = 0x329efb; continue;
            }
            0x329efb => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x329f04; continue;
            }
            0x329f04 => {
                esp = esp.wrapping_add(0x14);
                push!(0);
                push!(0);
                push!(0x1);
                call!(0x31f220);
                pc = 0x329f12; continue;
            }
            0x329f12 => {
                edx = m.read32(0x762f98);
                push!(eax);
                push!(0x1);
                edx = edx.wrapping_add(0xac);
                push!(edx);
                push!(0);
                push!(0);
                call!(0x7d5e0);
                pc = 0x329f2b; continue;
            }
            0x329f2b => {
                push!(0x109);
                call!(0x716d0);
                pc = 0x329f35; continue;
            }
            0x329f35 => {
                esp = esp.wrapping_add(0x24);
                call!(0x783d0);
                pc = 0x329f3d; continue;
            }
            0x329f3d => {
                push!(0);
                push!(0x329d60);
                push!(0x40000000);
                push!(0x800);
                push!(0xffffffffu32);
                push!(0xb63);
                call!(0x3d7b0);
                pc = 0x329f5a; continue;
            }
            0x329f5a => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x329f63; continue;
            }
            0x329f63 => {
                push!(0x10a);
                call!(0x716d0);
                pc = 0x329f6d; continue;
            }
            0x329f6d => {
                esp = esp.wrapping_add(0x1c);
                push!(0);
                push!(0x329da0);
                push!(0x40000000);
                push!(0x800);
                push!(0xffffffffu32);
                push!(0xb64);
                call!(0x3d7b0);
                pc = 0x329f8d; continue;
            }
            0x329f8d => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x329f96; continue;
            }
            0x329f96 => {
                push!(0x108);
                call!(0x716d0);
                pc = 0x329fa0; continue;
            }
            0x329fa0 => {
                esp = esp.wrapping_add(0x1c);
                push!(0);
                push!(0x329d00);
                push!(0x40000000);
                push!(0x800);
                push!(0xffffffffu32);
                push!(0xb65);
                call!(0x3d7b0);
                pc = 0x329fc0; continue;
            }
            0x329fc0 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x329fc9; continue;
            }
            0x329fc9 => {
                push!(0x10b);
                pc = 0x32a078; continue;
            }
            0x329fd3 => {
                push!(0xb62);
                call!(0x3d7b0);
                pc = 0x329fdd; continue;
            }
            0x329fdd => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x329fe6; continue;
            }
            0x329fe6 => {
                eax = m.read32(0x762f98);
                push!(0);
                ecx = eax.wrapping_add(0xa0);
                push!(ecx);
                push!(0x42a00000);
                push!(0x3f);
                eax = eax.wrapping_add(0x20);
                push!(eax);
                push!(0);
                push!(0);
                call!(0x7d560);
                pc = 0x32a008; continue;
            }
            0x32a008 => {
                push!(0x106);
                call!(0x716d0);
                pc = 0x32a012; continue;
            }
            0x32a012 => {
                esp = esp.wrapping_add(0x34);
                call!(0x783d0);
                pc = 0x32a01a; continue;
            }
            0x32a01a => {
                push!(0);
                push!(0x329d60);
                push!(0x40000000);
                push!(0x800);
                push!(0xffffffffu32);
                push!(0xb63);
                call!(0x3d7b0);
                pc = 0x32a037; continue;
            }
            0x32a037 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x32a040; continue;
            }
            0x32a040 => {
                push!(0x107);
                call!(0x716d0);
                pc = 0x32a04a; continue;
            }
            0x32a04a => {
                esp = esp.wrapping_add(0x1c);
                push!(0);
                push!(0x329da0);
                push!(0x40000000);
                push!(0x800);
                push!(0xffffffffu32);
                push!(0xb64);
                call!(0x3d7b0);
                pc = 0x32a06a; continue;
            }
            0x32a06a => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x32a073; continue;
            }
            0x32a073 => {
                push!(0x108);
                pc = 0x32a078; continue;
            }
            0x32a078 => {
                call!(0x716d0);
                pc = 0x32a07d; continue;
            }
            0x32a07d => {
                esp = esp.wrapping_add(0x1c);
                push!(0x40000000);
                call!(0x784a0);
                pc = 0x32a08a; continue;
            }
            0x32a08a => {
                esp = esp.wrapping_add(0x4);
                push!(0);
                push!(0x3254b0);
                push!(0x40000000);
                push!(0);
                push!(0xffffffffu32);
                push!(0xb5e);
                call!(0x3d7b0);
                pc = 0x32a0a7; continue;
            }
            0x32a0a7 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x32a0b0; continue;
            }
            0x32a0b0 => {
                call!(0x781a0);
                pc = 0x32a0b5; continue;
            }
            0x32a0b5 => {
                call!(0x22b90);
                pc = 0x32a0ba; continue;
            }
            0x32a0ba => {
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
                esp = esp.wrapping_add(0x14);
                m.write32(esp, xmm1.lanes[0]);
                push!(edx);
                call!(0x77320);
                pc = 0x32a105; continue;
            }
            0x32a105 => {
                esp = esp.wrapping_add(0x8);
                call!(0x75e70);
                pc = 0x32a10d; continue;
            }
            0x32a10d => {
                fa = m.read32(esi.wrapping_add(0x4));
                fb = ((((0x52e8b0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x32a1a2; continue; }
                pc = 0x32a11a; continue;
            }
            0x32a11a => {
                push!(0x2);
                push!(0x52e9e8);
                push!(esi);
                m.write32(esi.wrapping_add(0x218), 0x2);
                call!(0x78e50);
                pc = 0x32a131; continue;
            }
            0x32a131 => {
                edi = eax;
                call!(0x22b90);
                pc = 0x32a138; continue;
            }
            0x32a138 => {
                xmm0 = Xmm::scalar(m.read32(0x7ba188));
                xmm2 = xmm0;
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d30))).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x475d24))).to_bits();
                xmm1.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x7ba184))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x7ba030))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x475d18))).to_bits();
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x50)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x4760cc))).to_bits();
                esp = esp.wrapping_add(0x4);
                m.write32(esp.wrapping_add(0x4), xmm1.lanes[0]);
                m.write32(esp, xmm0.lanes[0]);
                push!(edi);
                call!(0x77360);
                pc = 0x32a198; continue;
            }
            0x32a198 => {
                push!(esi);
                push!(edi);
                call!(0x773b0);
                pc = 0x32a19f; continue;
            }
            0x32a19f => {
                esp = esp.wrapping_add(0x14);
                pc = 0x32a1a2; continue;
            }
            0x32a1a2 => {
                eax = m.read32(esi.wrapping_add(0x218));
                eax = (eax | 0x4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(esi.wrapping_add(0x218), eax);
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
