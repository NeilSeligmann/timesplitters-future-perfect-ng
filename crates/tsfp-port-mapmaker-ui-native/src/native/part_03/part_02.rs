use super::*; // portreg: replaces=tsfp-ports/mapmaker_ui_lane
#[rustfmt::skip] mod unformatted { use super::*;

/// 0x00318BE0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f318be0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x318be0;
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
        { crate::pose::cover(0x318be0, previous, pc); previous = pc; }
        match pc {
            0x318be0 => {
                push!(ebx);
                push!(esi);
                esi = m.read32(0x7844a8);
                push!(edi);
                esi = esi.wrapping_add(0x2f4c);
                call!(0x318b60);
                pc = 0x318bf4; continue;
            }
            0x318bf4 => {
                push!(0x2fb3c0);
                push!(0);
                push!(0x42480000);
                push!(0);
                push!(0x8400);
                call!(0x75b70);
                pc = 0x318c0c; continue;
            }
            0x318c0c => {
                ebx = m.read32(esp.wrapping_add(0x24));
                edi = 0x1;
                push!(edi);
                push!(edi);
                push!(0x3f000000);
                push!(edi);
                m.write32(ebx.wrapping_add(0x218), edi);
                call!(0x71b10);
                pc = 0x318c28; continue;
            }
            0x318c28 => {
                push!(0x20000);
                call!(0x71c50);
                pc = 0x318c32; continue;
            }
            0x318c32 => {
                esp = esp.wrapping_add(0x28);
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0xafe);
                call!(0x3d7b0);
                pc = 0x318c47; continue;
            }
            0x318c47 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x318c50; continue;
            }
            0x318c50 => {
                push!(0);
                push!(0);
                call!(0x7d4a0);
                pc = 0x318c59; continue;
            }
            0x318c59 => {
                push!(0x95);
                call!(0x716d0);
                pc = 0x318c63; continue;
            }
            0x318c63 => {
                esp = esp.wrapping_add(0x20);
                push!(0x2);
                push!(0x190d);
                call!(0x3d7b0);
                pc = 0x318c72; continue;
            }
            0x318c72 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x318c7b; continue;
            }
            0x318c7b => {
                esp = esp.wrapping_add(0x8);
                push!(edi);
                push!(0x190e);
                call!(0x3d7b0);
                pc = 0x318c89; continue;
            }
            0x318c89 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x318c92; continue;
            }
            0x318c92 => {
                eax = esi.wrapping_add(0x4);
                push!(eax);
                call!(0x78590);
                pc = 0x318c9b; continue;
            }
            0x318c9b => {
                esp = esp.wrapping_add(0xc);
                call!(0x783d0);
                pc = 0x318ca3; continue;
            }
            0x318ca3 => {
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0xa81);
                call!(0x3d7b0);
                pc = 0x318cb5; continue;
            }
            0x318cb5 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x318cbe; continue;
            }
            0x318cbe => {
                push!(0);
                push!(0);
                call!(0x7d4a0);
                pc = 0x318cc7; continue;
            }
            0x318cc7 => {
                push!(0x96);
                call!(0x716d0);
                pc = 0x318cd1; continue;
            }
            0x318cd1 => {
                esp = esp.wrapping_add(0x20);
                push!(0);
                push!(0xa84);
                call!(0x3d7b0);
                pc = 0x318ce0; continue;
            }
            0x318ce0 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x318ce9; continue;
            }
            0x318ce9 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x318cf0; continue;
            }
            0x318cf0 => {
                ecx = edi.wrapping_add(0xa84);
                push!(edi);
                push!(ecx);
                call!(0x3d7b0);
                pc = 0x318cfd; continue;
            }
            0x318cfd => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x318d06; continue;
            }
            0x318d06 => {
                esp = esp.wrapping_add(0x8);
                edi = edi.wrapping_add(1);
                fa = edi;
                fb = ((((0x5i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x318cf0; continue; }
                pc = 0x318d0f; continue;
            }
            0x318d0f => {
                edx = esi.wrapping_add(0x8);
                push!(edx);
                call!(0x78590);
                pc = 0x318d18; continue;
            }
            0x318d18 => {
                esp = esp.wrapping_add(0x4);
                call!(0x783d0);
                pc = 0x318d20; continue;
            }
            0x318d20 => {
                push!(0);
                push!(0);
                push!(0);
                push!(0x800);
                push!(0xffffffffu32);
                push!(0xa89);
                call!(0x3d7b0);
                pc = 0x318d37; continue;
            }
            0x318d37 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x318d40; continue;
            }
            0x318d40 => {
                push!(0);
                push!(0);
                call!(0x7d4f0);
                pc = 0x318d49; continue;
            }
            0x318d49 => {
                push!(0x97);
                call!(0x716d0);
                pc = 0x318d53; continue;
            }
            0x318d53 => {
                eax = esi.wrapping_add(0xc);
                push!(eax);
                call!(0x78560);
                pc = 0x318d5c; continue;
            }
            0x318d5c => {
                esp = esp.wrapping_add(0x28);
                push!(0);
                push!(0);
                push!(0);
                push!(0x800);
                push!(0xffffffffu32);
                push!(0xa8a);
                call!(0x3d7b0);
                pc = 0x318d76; continue;
            }
            0x318d76 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x318d7f; continue;
            }
            0x318d7f => {
                push!(0);
                push!(0);
                call!(0x7d4f0);
                pc = 0x318d88; continue;
            }
            0x318d88 => {
                push!(0x98);
                call!(0x716d0);
                pc = 0x318d92; continue;
            }
            0x318d92 => {
                esi = esi.wrapping_add(0x10);
                push!(esi);
                call!(0x78560);
                pc = 0x318d9b; continue;
            }
            0x318d9b => {
                push!(0x40000000);
                call!(0x784a0);
                pc = 0x318da5; continue;
            }
            0x318da5 => {
                esp = esp.wrapping_add(0x2c);
                push!(0);
                push!(0x2fb3c0);
                push!(0x40000000);
                push!(0);
                push!(0xffffffffu32);
                push!(0xb5e);
                call!(0x3d7b0);
                pc = 0x318dc2; continue;
            }
            0x318dc2 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x318dcb; continue;
            }
            0x318dcb => {
                call!(0x75e70);
                pc = 0x318dd0; continue;
            }
            0x318dd0 => {
                xmm0 = Xmm::scalar(m.read32(0x49f7e8));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ebx.wrapping_add(0x68)))).to_bits();
                ecx = m.read32(0x7bb1e8);
                esp = esp.wrapping_add(0x14);
                m.write32(esp, xmm0.lanes[0]);
                push!(ecx);
                call!(0x77320);
                pc = 0x318df1; continue;
            }
            0x318df1 => {
                esp = esp.wrapping_add(0x8);
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
}
pub use unformatted::*;
