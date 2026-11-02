// SPDX-License-Identifier: GPL-3.0-or-later
//! T1892 Tier 0: missing settings callback and two independently callable input suffixes.
//! INFERRED from original disassembly; smoke evidence only. See docs/t1892-small-systems.md.
use super::rt::*;

/// Settings/profile attachment record: guest addresses, not a repr(C) layout.
/// The 31-word viewport block is shared with gameplay/player; profile persistence
/// and mapping reload remain guest calls rather than invented host state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SettingsProfile {
    pub player: u32,
    pub viewport: u32,
    pub source: u32,
    pub destination: u32,
}

impl SettingsProfile {
    pub fn read<M: GuestMemory>(m: &M) -> Self {
        let player = m.read32(0x7b0c7c);
        let viewport = m.read32(0x7ba9e0);
        Self { player, viewport,
            source: 0x7ba364u32.wrapping_add(viewport.wrapping_mul(0x1e0)),
            destination: m.read32(player.wrapping_add(0x1500)) }
    }
}

/// 002E9E50: copy viewport settings to profile, reload controller mapping, pop page.
/// REP MOVSD advances forward under the lifted runtime's DF-clear contract.
/// Source/destination may overlap: preserve ordered read/write, not memcpy.
pub fn accept_settings<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, r: &mut ExtRegisters) {
    let profile = SettingsProfile::read(m);
    r.eax = profile.player;
    r.push(m, r.esi);
    r.esi = profile.source;
    r.push(m, r.edi);
    r.edi = profile.destination;
    r.ecx = 0x1f;
    while r.ecx != 0 {
        let word = m.read32(r.esi);
        m.write32(r.edi, word);
        r.esi = r.esi.wrapping_add(4);
        r.edi = r.edi.wrapping_add(4);
        r.ecx -= 1;
    }
    // Reload after the copy: aliasing is legal and the callee may change viewport.
    r.ecx = m.read32(0x7b0c7c);
    r.push(m, r.ecx);
    r.call(guest, 0xcf2d0);
    r.edx = m.read32(0x7ba9e0);
    r.eax = m.read32(0x761bbcu32.wrapping_add(r.edx.wrapping_mul(4)));
    r.esp = r.esp.wrapping_add(4);
    r.edi = m.read32(r.esp); r.esp = r.esp.wrapping_add(4);
    r.esi = m.read32(r.esp); r.esp = r.esp.wrapping_add(4);
    m.write32(r.esp.wrapping_add(4), r.eax);
    r.tail_call(guest, 0x79c00);
}

/// 0004276C: available-buttons suffix bypasses the 6B9888 input gate.
/// This is a distinct entry, not an alias to gated 00042760.
pub fn available_buttons_ungated<M: GuestMemory>(m: &M, r: &mut ExtRegisters) {
    r.eax = m.read32(0x76b0fc);
    if r.eax != 0 {
        r.ecx = m.read32(r.esp.wrapping_add(4)).wrapping_mul(0x3c);
        r.eax = m.read32(r.ecx.wrapping_add(0x6b9500));
        r.edx = m.read32(r.ecx.wrapping_add(0x6b94d4));
    } else {
        r.eax = m.read32(r.esp.wrapping_add(4));
        r.ecx = m.read32(0x6b93c0u32.wrapping_add(r.eax.wrapping_mul(4))).wrapping_mul(0x3c);
        r.eax = m.read32(r.ecx.wrapping_add(0x6b9408));
        r.edx = m.read32(r.ecx.wrapping_add(0x6b93dc));
    }
    r.eax = !r.eax & r.edx;
}

/// 00042BFC: record +34 suffix bypasses the 6B9888 gate; held/pressed/consumed
/// live elsewhere in PadRecord. The +34 field's domain name remains INFERRED.
pub fn word_34_ungated<M: GuestMemory>(m: &M, r: &mut ExtRegisters) {
    let alternate = m.read32(0x76b0fc) != 0;
    r.eax = m.read32(r.esp.wrapping_add(4));
    r.eax = if alternate {
        0x6b94c8u32.wrapping_add(r.eax.wrapping_mul(0x3c))
    } else {
        0x6b93d0u32.wrapping_add(m.read32(0x6b93c0u32.wrapping_add(r.eax.wrapping_mul(4))).wrapping_mul(0x3c))
    };
    r.eax = m.read32(r.eax.wrapping_add(0x34));
}

pub const ROOTS: [u32; 3] = [0x2e9e50, 0x4276c, 0x42bfc];

/// Opt-in per-function dispatch, without production admission or ABI changes.
pub fn run<M: GuestMemory, C: GuestCall>(va: u32, m: &mut M, guest: &mut C, r: &mut ExtRegisters) -> bool {
    match va {
        0x2e9e50 => accept_settings(m, guest, r),
        0x4276c => available_buttons_ungated(m, r),
        0x42bfc => word_34_ungated(m, r),
        _ => return false,
    }
    true
}
