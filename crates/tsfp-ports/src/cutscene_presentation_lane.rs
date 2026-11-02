// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: mirrors=tsfp-gameplay/cutscene_original_core
// portreg: mirrors=tsfp-gameplay/cutscene_overlays
// portreg: mirrors=tsfp-ports/system_slice/cutscene_camera_boundary
//! T2195: opt-in original movie-setup presentation attachment.
//! The guard confines guest access to the legacy ABI; arithmetic uses the readable model.
use crate::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
};
#[path = "system_slice/cutscene_camera_boundary.rs"]
mod camera_boundary;
#[path = "system_slice/cutscene_movie_frame_boundary.rs"]
mod movie_frame_boundary;
#[path = "system_slice/cutscene_overlay_boundary.rs"]
mod overlay_boundary;
unsafe extern "C" {
    fn getenv(name: *const core::ffi::c_char) -> *const core::ffi::c_char;
}

/// Unqualified presentation roots must retain the original under the owner's default `all`.
/// Explicit diagnostics additionally enable the original typed preset at process launch.
fn enabled() -> bool {
    unsafe {
        let value = getenv(OPT_IN.env.as_ptr());
        !value.is_null() && core::ffi::CStr::from_ptr(value).to_bytes() == OPT_IN.on.to_bytes()
    }
}

/// Admission belongs at the selector so a default-off root retains its original.
pub fn declined(va: u32) -> bool {
    ROOTS.contains(&va) && !enabled()
}

pub const ROOTS: [u32; 3] = [0x30210, 0x6f150, 0xc95e0];
/// The A/B switch of these roots (selector and T3738 `--t2-offline` opt-in registry).
pub const OPT_IN: crate::runtime::OptIn = crate::runtime::OptIn {
    env: c"TSFP_CUTSCENE_PRESENTATION",
    on: c"original",
    roots: &ROOTS,
};
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if !ROOTS.contains(&va) || !enabled() {
        return false;
    }
    crate::anim_lane::guarded(va, m, guest, regs, |view, guest, regs| {
        match va {
            0x30210 => crate::video::f30210(view, guest, regs),
            0x6f150 => overlay_boundary::draw_original(view, guest, regs),
            0xc95e0 => camera_boundary::apply_original(view, guest, regs),
            _ => unreachable!(),
        }
        true
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    struct DenyMemory;
    impl GuestMemory for DenyMemory {
        fn read8(&self, _: u32) -> u8 {
            panic!("unadmitted root read guest memory")
        }
        fn write8(&mut self, _: u32, _: u8) {
            panic!("unadmitted root wrote guest memory")
        }
    }
    struct DenyCalls;
    impl GuestCall for DenyCalls {
        fn call(&mut self, _: u32, _: &mut ExtRegisters) {
            panic!("unadmitted root called guest")
        }
    }
    #[test]
    fn unqualified_roots_retain_original_without_guest_access() {
        assert!(
            !enabled(),
            "run this admission check without TSFP_CUTSCENE_PRESENTATION"
        );
        let mut regs = ExtRegisters {
            eax: 11,
            ecx: 22,
            edx: 33,
            esp: 0,
            ebx: 44,
            ebp: 55,
            esi: 66,
            edi: 77,
            eflags: 0,
            fpu: crate::abi::Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0x27f,
            },
            xmm: [crate::abi::Xmm::default(); 5],
            xmm_hi: [crate::abi::Xmm::default(); 3],
        };
        let entry = regs;
        assert!(!declined(0xdeadbeef));
        for va in ROOTS {
            assert!(declined(va));
            assert!(!run(va, &mut DenyMemory, &mut DenyCalls, &mut regs));
            assert_eq!(regs, entry);
        }
    }
}
