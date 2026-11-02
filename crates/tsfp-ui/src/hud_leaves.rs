// SPDX-License-Identifier: GPL-3.0-or-later
//! T1836 wave 7 ui (hud and ui campaign) leaves. Each body ports a call-free C replacement of
//! `src/game`, see docs/t1836-rust-wave7.md. Every function takes the entry registers `e` and
//! returns the exact final EAX/ECX/EDX. Stack arguments are read through `esp`.
use crate::{gprs::Gprs, memory::GuestMemory};

const CAMPAIGN_STATUS: u32 = 0x0068_9178;
const CAMPAIGN_MISSING_RESULT: u32 = 0x0068_91ac;
const CAMPAIGN_READY_RESULT: u32 = 0x0068_9160;
const OVERLAY_COUNT: u32 = 0x0076_70e0;
const OVERLAY_RECORDS: u32 = 0x0076_6f60;
const OVERLAY_CAPACITY: i32 = 32;
const OVERLAY_RECORD_BYTES: u32 = 12;
const TRANSITION_SECOND: u32 = 0x0076_76dc;
const TRANSITION_FIRST: u32 = 0x0076_76c8;

/// 00032F10 game_ui_campaign_status_689178_write_result. cdecl(2): (slot_a, slot_b).
/// Status -1 writes [6891AC] to *slot_b, status 2 writes [689160] to *slot_a, EAX = status.
pub fn campaign_status_write_result<M: GuestMemory>(m: &mut M, esp: u32, e: Gprs) -> Gprs {
    let eax = m.read32(CAMPAIGN_STATUS);
    let (mut ecx, mut edx) = (e.ecx, e.edx);
    if eax == u32::MAX {
        ecx = m.read32(esp.wrapping_add(8));
        edx = m.read32(CAMPAIGN_MISSING_RESULT);
        m.write32(ecx, edx);
    } else if eax == 2 {
        ecx = m.read32(esp.wrapping_add(4));
        edx = m.read32(CAMPAIGN_READY_RESULT);
        m.write32(ecx, edx);
    }
    Gprs { eax, ecx, edx }
}

/// 00346250 game_hud_queue_overlay_request_12_bytes_max_0x20. cdecl(3): (a, b, c). Appends a
/// 12-byte record at 766F60 + 12*count unless the signed count is 32 or more. The second word
/// has its low byte cleared.
pub fn hud_queue_overlay_request<M: GuestMemory>(m: &mut M, esp: u32, e: Gprs) -> Gprs {
    let mut ecx = m.read32(OVERLAY_COUNT);
    if (ecx as i32) >= OVERLAY_CAPACITY {
        return Gprs {
            eax: e.eax,
            ecx,
            edx: e.edx,
        };
    }
    let mut edx = m.read32(esp.wrapping_add(4));
    let eax = ecx.wrapping_mul(OVERLAY_RECORD_BYTES);
    m.write32(eax.wrapping_add(OVERLAY_RECORDS), edx);
    edx = m.read32(esp.wrapping_add(8)) & 0xffff_ff00;
    m.write32(eax.wrapping_add(OVERLAY_RECORDS + 4), edx);
    edx = m.read32(esp.wrapping_add(12));
    ecx = ecx.wrapping_add(1);
    m.write32(eax.wrapping_add(OVERLAY_RECORDS + 8), edx);
    m.write32(OVERLAY_COUNT, ecx);
    Gprs { eax, ecx, edx }
}

/// 0034C0D0 game_hud_transition_effect_set_two_params_from_args. cdecl(2): (a, b).
pub fn hud_transition_set_two_params<M: GuestMemory>(m: &mut M, esp: u32, e: Gprs) -> Gprs {
    let eax = m.read32(esp.wrapping_add(8));
    let ecx = m.read32(esp.wrapping_add(4));
    m.write32(TRANSITION_SECOND, eax);
    m.write32(TRANSITION_FIRST, ecx);
    Gprs {
        eax,
        ecx,
        edx: e.edx,
    }
}
