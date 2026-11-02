// SPDX-License-Identifier: GPL-3.0-or-later
//! Vertex size from FVF flags (0x1E8D0). Lives apart from `leaves` so the lane build, whose crate root
//! already has a `leaves` module, can include it (T1976).
use crate::gprs::Gprs;

/// 0001E8D0 game_d3d_vertex_format_size_from_flags. cdecl(1): byte size of a vertex described
/// by the flexible-vertex-format flags.
// portreg: va=0x0001e8d0 tier=1
pub fn vertex_format_size(flags: u32, entry: Gprs) -> Gprs {
    let has = |bit: u32| flags & bit != 0;
    let mut eax = 0u32;
    if has(0x10) {
        eax += 4;
    }
    if has(0x1) {
        eax += 12;
    }
    if has(0x40) {
        eax += 12;
    } else if has(0x8000) {
        eax += 4;
    }
    if has(0x8) {
        eax += 4;
    }
    if has(0x20000) {
        eax += 4;
    }
    if has(0x4) {
        eax += 8;
    } else if has(0x80) {
        eax += 12;
    }
    if has(0x200) {
        eax += 8;
    } else if has(0x1000) {
        eax += 12;
    }
    if has(0x400) {
        eax += 8;
    } else if has(0x2000) {
        eax += 12;
    }
    if has(0x800) {
        eax += 8;
    } else if has(0x4000) {
        eax += 12;
    }
    if has(0x20) {
        eax += 8;
    }
    if has(0x100) {
        eax += 24;
    }
    if has(0x10000) {
        eax += 8;
    }
    Gprs { eax, ecx: flags, edx: entry.edx }
}
