// SPDX-License-Identifier: GPL-3.0-or-later
// T2214 test-only translated oracle, tools.c2rust at 274c63185. Never host-admitted.
use tsfp_ports::c2r::rt::{RegsExt, XmmExt};
use tsfp_ports::{
    abi::{ExtRegisters, GuestCall, Xmm},
    memory::GuestMemory,
};
/// `game_viewport_setup_for_player_slot_split_screen`: original 0x25A570 (?), lifted `sub_0025A570`.
/// Calls through the `GuestCall` trampoline to the retained original: 0x356800, 0x55E40, 0x1CDAE0, 0x55D90.
/// Register convention: `ExtRegisters` in and out, cdecl stack arguments read from guest memory.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
pub fn game_viewport_setup_for_player_slot_split_screen<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) {
    let (mut jt,) = (0,); // switch target
    r.push(m, r.ecx);
    r.eax = m.read32(r.esp.wrapping_add(8));
    r.push(m, r.ebx);
    r.ebx = m.read32(0x4e8cf0);
    r.push(m, r.ebp);
    r.push(m, r.esi);
    r.push(m, r.edi);
    r.edi = m.read32(0x4e8cf4);
    r.push(m, r.eax);
    r.ebx = ((r.ebx as i32).wrapping_shr(1)) as u32;
    r.edi = ((r.edi as i32).wrapping_shr(1)) as u32;
    r.call(guest, 0x356800);
    r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(0x4d1920)).to_bits());
    r.xmm[0].set_f(0, r.xmm[0].f(0) + f32::from_bits(m.read32(0x4760a8)));
    r.xmm[1] = Xmm::default();
    r.ebp = m.read32(r.esp.wrapping_add(0x20));
    r.esp = r.esp.wrapping_add(4);
    r.push(m, 0x43800000);
    r.push(m, r.ecx);
    m.write32(r.esp.wrapping_add(0x18), r.xmm[0].f(0).to_bits());
    r.ecx = m.read32(r.esp.wrapping_add(0x18));
    r.xmm[1].set_f(0, r.xmm[1].f(0) - r.xmm[0].f(0));
    m.write32(r.esp, r.xmm[1].f(0).to_bits());
    r.push(m, r.ecx);
    r.push(m, r.ebp);
    r.esi = r.eax;
    r.call(guest, 0x55e40);
    r.esp = r.esp.wrapping_add(0x10);
    r.call(guest, 0x1cdae0);
    'at_25a764: {
        r.eax = r.eax.wrapping_sub(1);
        if r.eax <= 3 {
            jt = m.read32(r.eax.wrapping_mul(4).wrapping_add(0x25a76c));
            if jt == 0x25a5e4 {
                if (((m.read8(0x7de458) as u32) & 0xff) & (0x40 & 0xff)) != 0 {
                    if m.read32(0x7de470) == 0x13 {
                        r.edx = m.read32(0x4e8cf4);
                        r.edx = r.edx.wrapping_sub(1);
                        r.push(m, r.edx);
                        r.ebx = r.ebx.wrapping_sub(1);
                        r.push(m, r.ebx);
                        r.push(m, 0);
                        r.push(m, 0);
                        r.push(m, r.ebp);
                        r.call(guest, 0x55d90);
                        r.esp = r.esp.wrapping_add(0x14);
                        r.edi = m.read32(r.esp);
                        r.esp = r.esp.wrapping_add(4);
                        r.esi = m.read32(r.esp);
                        r.esp = r.esp.wrapping_add(4);
                        r.ebp = m.read32(r.esp);
                        r.esp = r.esp.wrapping_add(4);
                        r.ebx = m.read32(r.esp);
                        r.esp = r.esp.wrapping_add(4);
                        r.ecx = m.read32(r.esp);
                        r.esp = r.esp.wrapping_add(4);
                        return;
                    }
                }
                // 0x25a613
                r.eax = m.read32(0x4e8cf4);
                r.ecx = m.read32(0x4e8cf0);
                r.eax = r.eax.wrapping_sub(1);
                r.push(m, r.eax);
                r.ecx = r.ecx.wrapping_sub(1);
                r.push(m, r.ecx);
                r.push(m, 0);
                r.push(m, 0);
                r.push(m, r.ebp);
                r.call(guest, 0x55d90);
                r.esp = r.esp.wrapping_add(0x14);
                r.edi = m.read32(r.esp);
                r.esp = r.esp.wrapping_add(4);
                r.esi = m.read32(r.esp);
                r.esp = r.esp.wrapping_add(4);
                r.ebp = m.read32(r.esp);
                r.esp = r.esp.wrapping_add(4);
                r.ebx = m.read32(r.esp);
                r.esp = r.esp.wrapping_add(4);
                r.ecx = m.read32(r.esp);
                r.esp = r.esp.wrapping_add(4);
                return;
            } else {
                if jt == 0x25a635 {
                    r.eax = m.read32(r.esp.wrapping_add(0x18));
                    r.eax = r.eax.wrapping_sub(0);
                    if r.eax == 0 {
                        r.ecx = m.read32(0x4e8cf0);
                        r.edi = r.edi.wrapping_sub(1);
                        r.push(m, r.edi);
                        r.ecx = r.ecx.wrapping_sub(1);
                        r.push(m, r.ecx);
                        r.push(m, 0);
                        r.push(m, 0);
                        r.push(m, r.ebp);
                        r.call(guest, 0x55d90);
                        r.esp = r.esp.wrapping_add(0x14);
                        r.edi = m.read32(r.esp);
                        r.esp = r.esp.wrapping_add(4);
                        r.esi = m.read32(r.esp);
                        r.esp = r.esp.wrapping_add(4);
                        r.ebp = m.read32(r.esp);
                        r.esp = r.esp.wrapping_add(4);
                        r.ebx = m.read32(r.esp);
                        r.esp = r.esp.wrapping_add(4);
                        r.ecx = m.read32(r.esp);
                        r.esp = r.esp.wrapping_add(4);
                        return;
                    } else {
                        r.eax = r.eax.wrapping_sub(1);
                        if r.eax == 0 {
                            r.edx = m.read32(0x4e8cf4);
                            r.eax = m.read32(0x4e8cf0);
                            r.edx = r.edx.wrapping_sub(1);
                            r.push(m, r.edx);
                            r.eax = r.eax.wrapping_sub(1);
                            r.push(m, r.eax);
                            r.push(m, r.edi);
                            r.push(m, 0);
                            r.push(m, r.ebp);
                            r.call(guest, 0x55d90);
                            r.esp = r.esp.wrapping_add(0x14);
                            r.edi = m.read32(r.esp);
                            r.esp = r.esp.wrapping_add(4);
                            r.esi = m.read32(r.esp);
                            r.esp = r.esp.wrapping_add(4);
                            r.ebp = m.read32(r.esp);
                            r.esp = r.esp.wrapping_add(4);
                            r.ebx = m.read32(r.esp);
                            r.esp = r.esp.wrapping_add(4);
                            r.ecx = m.read32(r.esp);
                            r.esp = r.esp.wrapping_add(4);
                            return;
                        }
                    }
                } else {
                    if jt == 0x25a683 {
                        r.eax = m.read32(r.esp.wrapping_add(0x18));
                        r.eax = r.eax.wrapping_sub(0);
                        if r.eax == 0 {
                            r.eax = m.read32(0x4e8cf0);
                            r.edi = r.edi.wrapping_sub(1);
                            r.push(m, r.edi);
                            r.eax = r.eax.wrapping_sub(1);
                            r.push(m, r.eax);
                            r.push(m, 0);
                            r.push(m, 0);
                            r.push(m, r.ebp);
                            r.call(guest, 0x55d90);
                            r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(0x478900)).to_bits());
                        } else {
                            r.eax = r.eax.wrapping_sub(1);
                            if r.eax == 0 {
                                r.eax = m.read32(0x4e8cf4);
                                r.eax = r.eax.wrapping_sub(1);
                                r.push(m, r.eax);
                                r.ebx = r.ebx.wrapping_sub(1);
                                r.push(m, r.ebx);
                                r.push(m, r.edi);
                                r.push(m, 0);
                            } else {
                                r.eax = r.eax.wrapping_sub(1);
                                if r.eax != 0 {
                                    break 'at_25a764;
                                }
                                r.edx = m.read32(0x4e8cf4);
                                r.eax = m.read32(0x4e8cf0);
                                r.edx = r.edx.wrapping_sub(1);
                                r.push(m, r.edx);
                                r.eax = r.eax.wrapping_sub(1);
                                r.push(m, r.eax);
                                r.push(m, r.edi);
                                r.push(m, r.ebx);
                            }
                            // 0x25a6b5
                            r.push(m, r.ebp);
                            r.call(guest, 0x55d90);
                            r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(0x4785cc)).to_bits());
                        }
                        // 0x25a6c3
                        r.ecx = m.read32(0x7b0c48);
                        r.esp = r.esp.wrapping_add(0x14);
                        r.esi = ((r.esi as i32).wrapping_mul(0x1584i32)) as u32;
                        r.edx = m.read32(r.esi.wrapping_add(r.ecx).wrapping_add(8));
                        r.edi = m.read32(r.esp);
                        r.esp = r.esp.wrapping_add(4);
                        r.esi = m.read32(r.esp);
                        r.esp = r.esp.wrapping_add(4);
                        r.ebp = m.read32(r.esp);
                        r.esp = r.esp.wrapping_add(4);
                        m.write32(r.edx.wrapping_add(0x384), r.xmm[0].f(0).to_bits());
                        r.ebx = m.read32(r.esp);
                        r.esp = r.esp.wrapping_add(4);
                        r.ecx = m.read32(r.esp);
                        r.esp = r.esp.wrapping_add(4);
                        return;
                    }
                    if jt != 0x25a701 {
                        r.tail_call(guest, jt); // indirect tail jump, target outside the switch
                        return;
                    }
                    r.eax = m.read32(r.esp.wrapping_add(0x18));
                    r.edx = m.read32(0x4e8cf4);
                    r.ebx = r.eax;
                    r.ebx = r.ebx & 2;
                    if r.ebx == 0 {
                        r.esi = r.edx;
                        r.esi = ((r.esi as i32).wrapping_shr(1)) as u32;
                        r.esi = r.esi.wrapping_sub(1);
                    } else {
                        r.esi = r.edx.wrapping_add((1i32.wrapping_neg()) as u32);
                    }
                    // 0x25a71c
                    r.edi = r.eax;
                    r.edi = r.edi & 1;
                    r.eax = m.read32(0x4e8cf0);
                    if r.edi == 0 {
                        r.ecx = r.eax;
                        r.ecx = ((r.ecx as i32).wrapping_shr(1)) as u32;
                        r.ecx = r.ecx.wrapping_sub(1);
                    } else {
                        r.ecx = r.eax.wrapping_add((1i32.wrapping_neg()) as u32);
                    }
                    if r.ebx == 0 {
                        r.edx = 0;
                    } else {
                        r.edx = ((r.edx as i32).wrapping_shr(1)) as u32;
                    }
                    if r.edi != 0 {
                        r.push(m, r.esi);
                        r.push(m, r.ecx);
                        r.push(m, r.edx);
                        r.eax = ((r.eax as i32).wrapping_shr(1)) as u32;
                        r.push(m, r.eax);
                        r.push(m, r.ebp);
                        r.call(guest, 0x55d90);
                        r.esp = r.esp.wrapping_add(0x14);
                        r.edi = m.read32(r.esp);
                        r.esp = r.esp.wrapping_add(4);
                        r.esi = m.read32(r.esp);
                        r.esp = r.esp.wrapping_add(4);
                        r.ebp = m.read32(r.esp);
                        r.esp = r.esp.wrapping_add(4);
                        r.ebx = m.read32(r.esp);
                        r.esp = r.esp.wrapping_add(4);
                        r.ecx = m.read32(r.esp);
                        r.esp = r.esp.wrapping_add(4);
                        return;
                    }
                    r.push(m, r.esi);
                    r.push(m, r.ecx);
                    r.push(m, r.edx);
                    r.eax = 0;
                    r.push(m, r.eax);
                    r.push(m, r.ebp);
                    r.call(guest, 0x55d90);
                    r.esp = r.esp.wrapping_add(0x14);
                }
            }
        }
    }
    // 0x25a764
    r.edi = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
    r.esi = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
    r.ebp = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
    r.ebx = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
    r.ecx = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
}

/// `game_input_pad_map_find_index`: original 0x426B0 (?), lifted `sub_000426B0`.
/// Register convention: `ExtRegisters` in and out, cdecl stack arguments read from guest memory.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
pub fn game_input_pad_map_find_index<M: GuestMemory>(m: &mut M, r: &mut ExtRegisters) {
    r.ecx = m.read32(r.esp.wrapping_add(4));
    r.eax = 0;
    loop {
        if m.read32(r.eax.wrapping_mul(4).wrapping_add(0x6b93c0)) == r.ecx {
            return;
        }
        r.eax = r.eax.wrapping_add(1);
        if (r.eax as i32) >= 4 {
            r.eax = r.eax | 0xffffffffu32;
            return;
        }
    }
}

/// `game_input_pad_map_get`: original 0x426D0 (?), lifted `sub_000426D0`.
/// Register convention: `ExtRegisters` in and out, cdecl stack arguments read from guest memory.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
pub fn game_input_pad_map_get<M: GuestMemory>(m: &mut M, r: &mut ExtRegisters) {
    r.eax = m.read32(r.esp.wrapping_add(4));
    r.eax = m.read32(r.eax.wrapping_mul(4).wrapping_add(0x6b93c0));
}
