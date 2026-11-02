// SPDX-License-Identifier: GPL-3.0-or-later
//! T1836 wave 8 physics leaves that use scalar SSE. Each body ports a proven C replacement of
//! `src/game`, see docs/t1836-rust-wave8.md. A function takes the guest memory, the entry `esp`
//! (return address at `esp`, cdecl arguments above it), the entry registers and the eight XMM
//! registers, and returns the exact final EAX/ECX/EDX. XMM state is updated in place.
use crate::{
    gprs::Gprs,
    memory::GuestMemory,
    sse::{self, Xmm},
};

fn arg<M: GuestMemory>(m: &M, esp: u32, index: u32) -> u32 {
    m.read32(esp.wrapping_add(4 + 4 * index))
}

/// 00047B30 game_ray_init_from_origin_direction_length. cdecl(4): (ray, origin, direction, length).
/// Copies origin to ray+0, direction to ray+12, stores length at ray+36, then the reciprocal of
/// each direction component at ray+24, +28, +32 (0 when the component compares ordered equal to 0).
/// EAX keeps the low byte and high word of the last direction word read, AH is the ucomiss flag
/// byte of the last compare. ECX = ray, EDX = direction. xmm0..2 are overwritten.
// portreg: va=0x00047b30 tier=1
pub fn ray_init_from_origin_direction_length<M: GuestMemory>(
    m: &mut M,
    esp: u32,
    _e: Gprs,
    x: &mut [Xmm; 8],
) -> Gprs {
    let ecx = arg(m, esp, 0);
    let mut eax = arg(m, esp, 1);
    let mut edx = m.read32(eax);
    x[0] = [m.read32(esp.wrapping_add(16)), 0, 0, 0];
    x[1] = [0; 4];
    x[2] = [m.read32(0x0047_5c78), 0, 0, 0];
    m.write32(ecx, edx);
    edx = m.read32(eax.wrapping_add(4));
    m.write32(ecx.wrapping_add(4), edx);
    eax = m.read32(eax.wrapping_add(8));
    edx = arg(m, esp, 2);
    m.write32(ecx.wrapping_add(8), eax);
    eax = m.read32(edx);
    m.write32(ecx.wrapping_add(12), eax);
    eax = m.read32(edx.wrapping_add(4));
    m.write32(ecx.wrapping_add(16), eax);
    eax = m.read32(edx.wrapping_add(8));
    m.write32(ecx.wrapping_add(20), eax);
    m.write32(ecx.wrapping_add(36), x[0][0]);
    for i in 0..2u32 {
        let component = edx.wrapping_add(4 * i);
        x[0] = [m.read32(component), 0, 0, 0];
        let flags = sse::ucomiss(x[0][0], x[1][0]);
        eax = flags.lahf_into(eax);
        if flags.ordered_equal() {
            x[0] = x[1];
        } else {
            x[0] = x[2];
            x[0][0] = sse::divss(x[0][0], m.read32(component));
        }
        m.write32(ecx.wrapping_add(24 + 4 * i), x[0][0]);
    }
    x[0] = [m.read32(edx.wrapping_add(8)), 0, 0, 0];
    let flags = sse::ucomiss(x[0][0], x[1][0]);
    eax = flags.lahf_into(eax);
    if flags.ordered_equal() {
        m.write32(ecx.wrapping_add(32), x[1][0]);
    } else {
        x[2][0] = sse::divss(x[2][0], m.read32(edx.wrapping_add(8)));
        m.write32(ecx.wrapping_add(32), x[2][0]);
    }
    Gprs { eax, ecx, edx }
}
