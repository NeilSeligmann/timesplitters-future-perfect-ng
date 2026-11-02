// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: mirrors=tsfp-ports/c2r/t2049_exec_005
// portreg: mirrors=tsfp-engine-physics/fp_vector
// portreg: mirrors=tsfp-gameplay/mode_leaves
// portreg: mirrors=tsfp-ports/c2r/player2_wired
// portreg: mirrors=tsfp-ports/c2r/t2049_nx_006
//! T1831 weapons attachment checkpoint. Every root is Tier 1; opt-in, not admitted.
//! Lifted differential on FABRICATED inputs and stub callees (3000 requested seeds,
//! 1485-3000 clean/root at O0/O3). Names are inferred; no live closure/xemu proof.
//! Evidence and reproduction: docs/t1831-remote-weapons.md.
use crate::{
    abi::{ExtRegisters, GuestCall, Xmm},
    memory::GuestMemory,
};

/// Shared guest records remain authoritative; addresses wrap as guest u32 values.
#[derive(Clone, Copy)]
pub struct WeaponSlot(pub u32);
impl WeaponSlot {
    pub fn field(self, offset: u32) -> u32 {
        self.0.wrapping_add(offset)
    }
}
/// Weapons touches mode state shared with gameplay/player and network/session.
pub const MODE: u32 = 0x7de455;
pub const DEFINITIONS: u32 = 0x4f9bac;
// portreg: va=0x000c5c30 tier=1
// portreg: va=0x001b5680 tier=1
// portreg: va=0x001b58c0 tier=1
// portreg: va=0x000bc180 tier=1
// portreg: va=0x000bc2e0 tier=1
// portreg: va=0x000bc470 tier=1
// portreg: va=0x000bc4a0 tier=1
// portreg: va=0x000b9300 tier=1
// portreg: va=0x000bfda0 tier=1
// portreg: va=0x001b7bc0 tier=1
pub const ROOTS: &[u32] = &[
    0xc5c30, 0x1b5680, 0x1b58c0, 0xbc180, 0xbc2e0, 0xbc470, 0xbc4a0, 0xb9300, 0xbfda0, 0x1b7bc0,
];
pub const VERIFICATION_TIER: u8 = 1;
fn arg<M: GuestMemory>(m: &M, r: &ExtRegisters, n: u32) -> u32 {
    m.read32(r.esp.wrapping_add(4 + n * 4))
}
fn push<M: GuestMemory>(m: &mut M, r: &mut ExtRegisters, v: u32) {
    r.esp = r.esp.wrapping_sub(4);
    m.write32(r.esp, v);
}
fn pop<M: GuestMemory>(m: &M, r: &mut ExtRegisters) -> u32 {
    let v = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
    v
}
/// Tail transfer through GuestCall: preserve the original return slot and propagate callee state.
fn tail<M: GuestMemory, C: GuestCall>(m: &mut M, c: &mut C, r: &mut ExtRegisters, va: u32) {
    let slot = r.esp;
    let saved = m.read32(slot);
    r.esp = r.esp.wrapping_add(4);
    c.call(va, r);
    r.esp = r.esp.wrapping_sub(4);
    m.write32(slot, saved);
}
/// Per-address hybrid switch: false leaves memory, registers and calls untouched.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    enabled: bool,
    m: &mut M,
    c: &mut C,
    r: &mut ExtRegisters,
) -> bool {
    if !enabled || !ROOTS.contains(&va) {
        return false;
    }
    match va {
        0xc5c30 => r.eax = 0x733094,
        0x1b5680 => {
            r.eax = arg(m, r, 0);
            r.ecx = arg(m, r, 1);
            r.xmm[0] = Xmm::default();
            m.write32(r.eax.wrapping_add(0x44), r.ecx);
            m.write32(r.eax.wrapping_add(0x48), 0);
        }
        0x1b58c0 => {
            r.eax = arg(m, r, 0);
            r.ecx = m.read32(r.eax.wrapping_add(0x30)).wrapping_mul(0x29c);
            r.edx = m.read32(DEFINITIONS);
            r.eax = m.read32(r.ecx.wrapping_add(r.edx).wrapping_add(0xc)) & 0x80;
        }
        0xbc180 | 0xbc2e0 | 0xbc470 | 0xbc4a0 => {
            r.eax = arg(m, r, 0);
            let slot = WeaponSlot(r.eax);
            let mode = m.read8(MODE);
            let value = match va {
                0xbc470 => Some(0),
                0xbc4a0 => Some(m.read32(0x47862c)),
                0xbc180 if m.read32(slot.field(0x18)) == 0x1a => Some(0),
                0xbc2e0 if mode == 0xa && m.read32(slot.field(0x24)) == 0x18 => {
                    Some(m.read32(0x475d18))
                }
                _ => None,
            };
            if let Some(bits) = value {
                r.xmm[0] = Xmm::scalar(bits);
                m.write32(slot.field(0x20), bits);
            }
            if va == 0xbc2e0 && mode == 0xa && value.is_none() {
                return true;
            }
            if mode == 0xa {
                m.write32(r.esp.wrapping_add(4), r.eax);
            }
            tail(m, c, r, if mode == 0xa { 0xba230 } else { 0xba310 });
        }
        0xb9300 => {
            if m.read32(r.eax.wrapping_add(0xc6c)) & 0x40000 != 0 {
                r.eax = 0;
            } else if m.read32(0x79094c) != 9 {
                r.eax = 1;
            } else {
                r.eax = m.read32(r.eax.wrapping_add(0xbf0));
                r.eax = if r.eax != 0 && m.read32(r.eax.wrapping_add(8)) == 0x42524757 {
                    0
                } else {
                    1
                };
            }
        }
        0xbfda0 => {
            r.esp = r.esp.wrapping_sub(0x30);
            r.eax = r.esp;
            push(m, r, r.eax);
            c.call(0x45390, r);
            r.xmm[0] = Xmm::scalar(m.read32(r.esi));
            m.write32(r.esp.wrapping_add(0x20), r.xmm[0].lanes[0]);
            r.xmm[0] = Xmm::scalar(m.read32(r.esi.wrapping_add(4)));
            r.ecx = r.esp.wrapping_add(4);
            m.write32(r.esp.wrapping_add(0x24), r.xmm[0].lanes[0]);
            r.xmm[0] = Xmm::scalar(m.read32(r.esi.wrapping_add(8)));
            push(m, r, r.ecx);
            m.write8(r.esp.wrapping_add(0x30), 1);
            m.write32(r.esp.wrapping_add(8), 0xd6d);
            m.write32(r.esp.wrapping_add(0x2c), r.xmm[0].lanes[0]);
            c.call(0x45820, r);
            r.esp = r.esp.wrapping_add(0x38);
        }
        0x1b7bc0 => {
            let ext = arg(m, r, 0);
            let controller = arg(m, r, 1);
            let hand = arg(m, r, 2);
            let saved_round_limit = arg(m, r, 3) != 0;
            // T1678: the saved-round restore caller clears all 44 loaded words before entering
            // this wrapper with argument 4 set. Keep the retained T2411 owner in that same
            // pre-transfer state before the owned transfer below runs.
            if saved_round_limit {
                let _ =
                    crate::weapon_hands_runtime::clear_owned_hand_for_saved_restore(m, ext, hand);
            }
            r.eax = saved_round_limit as u32;
            r.ecx = ext;
            push(m, r, r.esi);
            r.esi = m.read32(r.esp.wrapping_add(0xc));
            push(m, r, r.edi);
            r.edi = m.read32(r.esp.wrapping_add(0x14));
            push(m, r, r.eax);
            push(m, r, r.esi);
            push(m, r, r.ecx);
            r.eax = r.edi;
            if !crate::weapon_hands_runtime::enabled()
                || !crate::weapon_hands_runtime::transfer_from_runtime_owned_store_args(
                    m,
                    ext,
                    controller,
                    hand,
                    saved_round_limit,
                    r,
                )
            {
                c.call(0x1b6ec0, r);
            }
            r.xmm[0] = Xmm::default();
            r.eax = 0;
            for offset in [0xd8, 0xc0, 0xb0, 0x5c, 0x80, 0xbc, 0xb8] {
                m.write32(r.esi.wrapping_add(offset), 0);
            }
            m.write32(r.esi.wrapping_add(0xa8), u32::MAX);
            r.edx = m.read32(0x7b0c7c);
            m.write32(r.edx.wrapping_add(0xfb0), 0);
            r.ecx = 1000;
            r.esp = r.esp.wrapping_add(0xc);
            m.write32(r.edi.wrapping_add(0xf0), 1000);
            m.write32(r.edi.wrapping_add(0xf4), 1000);
            m.write32(r.edi.wrapping_add(0x104), 0);
            m.write32(r.edi.wrapping_add(0x10c), 0);
            r.edi = pop(m, r);
            r.esi = pop(m, r);
        }
        _ => unreachable!(),
    }
    true
}
