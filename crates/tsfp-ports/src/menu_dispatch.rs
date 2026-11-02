// SPDX-License-Identifier: GPL-3.0-or-later
//! 0x1AF950 `game_menu_message_dispatch_by_code_play_click_sound_and_push_page_descriptor_widget_jump_table`
//! (T2042), hand port. cdecl(object, code); the function returns nothing.
//!
//! The original is a 0..0xe5 jump table (673 lifted blocks) in which almost every arm is the same four steps with a
//! different page descriptor address, so the port is a data table plus two arm bodies:
//!
//! * `Std`: play the click when `[MenuState.click_sound_enabled]` is set (`0x21780(*[click_sound_request])`), then
//!   push the descriptor as a child page with `0x79b70(ui_state_object, descriptor)`.
//! * `Alloc`: no click; `0x78e50(object, descriptor, 2)` creates or finds the child widget, `[0x756f0c] = 1`, and
//!   `0x773b0(object, child)` runs the open transition.
//! * Five codes call one helper instead: 0x43 `0x196050(object)`, 0x90 `0x79c00(ui_state_object)` (pop context),
//!   0x99 `0x389b0(object)`, 0xac `0x1a08b0(object)`, 0xdd `0x1a1490(object)`. Any other code returns at once.
//!   (The Ghidra export shows `0x389b0()` and `0x1a1490()` without arguments and 0xac as the thunk `0x2ec2f0`;
//!   the lifted original pushes the object for both and calls `0x1a08b0`, which is what the port follows.)
//!
//! The table below is generated from the Ghidra export by `python3 -m tools.t2042_menu_dispatch` (descriptor
//! addresses and arm kinds only; no code bytes). Several codes share one descriptor (for example 0x49, 0x4b and
//! 0x8e all open `0x4e9824`), which is kept as listed. Note: docs/t1468-menu-page-descriptors.md called 0x49, 0x4b,
//! 0x8a..0x8e and 0x90 "pop" arms; the export shows only 0x90 pops, the rest push descriptors.
//!
//! Verification tier: 1, OBSERVABLE-ONLY: `C2R_LOOSE=1 test_rust_c2r` compares this port against the lifted original
//! on the callee sequence, the callee arguments (by true arity), memory and the callee-saved registers, over all
//! codes 0..0xe5 (FABRICATED fixtures). Scratch registers (eax/ecx/edx at each call) and the stack residue the
//! original leaves are not reproduced; the strict mode only passes for the translated twin
//! `c2r::ui_menu_t2042_dispatch`, which stays as the reference and A/B fallback.
use crate::abi::{ExtRegisters, GuestCall, call_cdecl};
use crate::c2r::rt::RegsExt;
use crate::memory::GuestMemory;
use crate::menu_state::{MenuState, MenuStateView};

const PLAY_CLICK: u32 = 0x0002_1780;
const PUSH_PAGE: u32 = 0x0007_9b70;
const POP_PAGE: u32 = 0x0007_9c00;
const CREATE_OR_FIND_CHILD: u32 = 0x0007_8e50;
const OPEN_TRANSITION: u32 = 0x0007_73b0;
const CHILD_MODE: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArmKind {
    /// Click sound, then push the page descriptor on the context stack.
    Std,
    /// Create or find the child widget, set the transition flag, open it (no click).
    Alloc,
}

/// `(message code, arm kind, descriptor address)`, sorted by code.
pub static ARMS: [(u32, ArmKind, u32); 225] = [
    (0x00, ArmKind::Std, 0x4e92c4),
    (0x01, ArmKind::Std, 0x4e9634),
    (0x02, ArmKind::Std, 0x4e9564),
    (0x03, ArmKind::Std, 0x4e9d74),
    (0x04, ArmKind::Std, 0x4e9c54),
    (0x05, ArmKind::Std, 0x4e92e4),
    (0x06, ArmKind::Std, 0x4e9324),
    (0x07, ArmKind::Std, 0x4e9314),
    (0x08, ArmKind::Std, 0x4e9334),
    (0x09, ArmKind::Std, 0x4e9314),
    (0x0a, ArmKind::Std, 0x4e93a4),
    (0x0b, ArmKind::Std, 0x4e93b4),
    (0x0c, ArmKind::Std, 0x4e9384),
    (0x0d, ArmKind::Std, 0x4e94f4),
    (0x0e, ArmKind::Std, 0x4e9234),
    (0x0f, ArmKind::Std, 0x4e9354),
    (0x10, ArmKind::Std, 0x4e9364),
    (0x11, ArmKind::Std, 0x4e9344),
    (0x12, ArmKind::Std, 0x4e9574),
    (0x13, ArmKind::Std, 0x4e93c4),
    (0x14, ArmKind::Std, 0x4e94e4),
    (0x15, ArmKind::Std, 0x4e93d4),
    (0x16, ArmKind::Std, 0x4e94c4),
    (0x17, ArmKind::Std, 0x4e9414),
    (0x18, ArmKind::Alloc, 0x4e9df4),
    (0x19, ArmKind::Std, 0x4e9434),
    (0x1a, ArmKind::Std, 0x4e9444),
    (0x1b, ArmKind::Std, 0x4e9454),
    (0x1c, ArmKind::Std, 0x4e9474),
    (0x1d, ArmKind::Std, 0x4e9484),
    (0x1e, ArmKind::Std, 0x4e94b4),
    (0x1f, ArmKind::Alloc, 0x4e9e04),
    (0x20, ArmKind::Alloc, 0x4e9e14),
    (0x21, ArmKind::Alloc, 0x4e9e24),
    (0x22, ArmKind::Alloc, 0x4e9e34),
    (0x23, ArmKind::Alloc, 0x4e9e44),
    (0x24, ArmKind::Alloc, 0x4e9e54),
    (0x25, ArmKind::Alloc, 0x4e9e64),
    (0x26, ArmKind::Alloc, 0x4e9e74),
    (0x27, ArmKind::Std, 0x4e9394),
    (0x28, ArmKind::Std, 0x4e94c4),
    (0x29, ArmKind::Std, 0x4e93e4),
    (0x2a, ArmKind::Std, 0x4e9824),
    (0x2b, ArmKind::Std, 0x4e9584),
    (0x2c, ArmKind::Std, 0x4e9584),
    (0x2d, ArmKind::Std, 0x4e9524),
    (0x2e, ArmKind::Std, 0x4e9544),
    (0x2f, ArmKind::Std, 0x4e98f4),
    (0x30, ArmKind::Std, 0x4e9534),
    (0x31, ArmKind::Std, 0x4e9824),
    (0x32, ArmKind::Std, 0x4e9554),
    (0x33, ArmKind::Std, 0x4e9584),
    (0x34, ArmKind::Std, 0x4e9584),
    (0x35, ArmKind::Std, 0x4e95c4),
    (0x36, ArmKind::Std, 0x4e95b4),
    (0x37, ArmKind::Std, 0x4e95d4),
    (0x38, ArmKind::Std, 0x4e99d4),
    (0x39, ArmKind::Std, 0x4e9614),
    (0x3a, ArmKind::Std, 0x4e95f4),
    (0x3b, ArmKind::Alloc, 0x4e9e84),
    (0x3c, ArmKind::Std, 0x4e9614),
    (0x3d, ArmKind::Std, 0x4e9604),
    (0x3e, ArmKind::Std, 0x4e9824),
    (0x3f, ArmKind::Std, 0x4e99c4),
    (0x40, ArmKind::Std, 0x4e99d4),
    (0x41, ArmKind::Std, 0x4e9b54),
    (0x42, ArmKind::Std, 0x4e9704),
    (0x44, ArmKind::Std, 0x4e9654),
    (0x45, ArmKind::Std, 0x4e9644),
    (0x46, ArmKind::Std, 0x4e9694),
    (0x47, ArmKind::Std, 0x4e9664),
    (0x48, ArmKind::Std, 0x4e9674),
    (0x49, ArmKind::Std, 0x4e9824),
    (0x4a, ArmKind::Std, 0x4e9684),
    (0x4b, ArmKind::Std, 0x4e9824),
    (0x4c, ArmKind::Std, 0x4e9634),
    (0x4d, ArmKind::Std, 0x4e96a4),
    (0x4e, ArmKind::Std, 0x4e97e4),
    (0x4f, ArmKind::Std, 0x4e96b4),
    (0x50, ArmKind::Std, 0x4e96d4),
    (0x51, ArmKind::Std, 0x4e96c4),
    (0x52, ArmKind::Std, 0x4e96e4),
    (0x53, ArmKind::Std, 0x4e97c4),
    (0x54, ArmKind::Std, 0x4e9614),
    (0x55, ArmKind::Std, 0x4e96d4),
    (0x56, ArmKind::Std, 0x4e96d4),
    (0x57, ArmKind::Alloc, 0x4e9e94),
    (0x58, ArmKind::Alloc, 0x4e9ea4),
    (0x59, ArmKind::Std, 0x4e96f4),
    (0x5a, ArmKind::Std, 0x4e9714),
    (0x5b, ArmKind::Alloc, 0x4e9eb4),
    (0x5c, ArmKind::Alloc, 0x4e9ec4),
    (0x5d, ArmKind::Alloc, 0x4e9ec4),
    (0x5e, ArmKind::Std, 0x4e97c4),
    (0x5f, ArmKind::Std, 0x4e9724),
    (0x60, ArmKind::Std, 0x4e9734),
    (0x61, ArmKind::Std, 0x4e9744),
    (0x62, ArmKind::Std, 0x4e96d4),
    (0x63, ArmKind::Std, 0x4e9764),
    (0x64, ArmKind::Std, 0x4e96d4),
    (0x65, ArmKind::Std, 0x4e9774),
    (0x66, ArmKind::Std, 0x4e96d4),
    (0x67, ArmKind::Std, 0x4e9784),
    (0x68, ArmKind::Std, 0x4e96d4),
    (0x69, ArmKind::Alloc, 0x4e9ee4),
    (0x6a, ArmKind::Std, 0x4e97a4),
    (0x6b, ArmKind::Std, 0x4e9794),
    (0x6c, ArmKind::Alloc, 0x4e9ef4),
    (0x6d, ArmKind::Std, 0x5246ac),
    (0x6e, ArmKind::Std, 0x4e97c4),
    (0x6f, ArmKind::Std, 0x4e97d4),
    (0x70, ArmKind::Std, 0x4e8ea0),
    (0x71, ArmKind::Std, 0x4e97e4),
    (0x72, ArmKind::Std, 0x4e9a04),
    (0x73, ArmKind::Std, 0x4e99e4),
    (0x74, ArmKind::Std, 0x4e9894),
    (0x75, ArmKind::Std, 0x4e9834),
    (0x76, ArmKind::Std, 0x4e9844),
    (0x77, ArmKind::Std, 0x4e9854),
    (0x78, ArmKind::Std, 0x4e97c4),
    (0x79, ArmKind::Std, 0x4e9864),
    (0x7a, ArmKind::Std, 0x4e9874),
    (0x7b, ArmKind::Std, 0x4e9454),
    (0x7c, ArmKind::Std, 0x4e9454),
    (0x7d, ArmKind::Std, 0x4e92c4),
    (0x7e, ArmKind::Std, 0x4e98a4),
    (0x7f, ArmKind::Std, 0x4e98b4),
    (0x80, ArmKind::Std, 0x4e9934),
    (0x81, ArmKind::Std, 0x4e98e4),
    (0x82, ArmKind::Std, 0x4e9934),
    (0x83, ArmKind::Std, 0x4e9904),
    (0x84, ArmKind::Std, 0x4e9914),
    (0x85, ArmKind::Std, 0x4e9934),
    (0x86, ArmKind::Std, 0x4e98d4),
    (0x87, ArmKind::Std, 0x4e9924),
    (0x88, ArmKind::Std, 0x4e9944),
    (0x89, ArmKind::Std, 0x4e99d4),
    (0x8a, ArmKind::Std, 0x4e9974),
    (0x8b, ArmKind::Std, 0x4e9584),
    (0x8c, ArmKind::Std, 0x4e92c4),
    (0x8d, ArmKind::Std, 0x4e92c4),
    (0x8e, ArmKind::Std, 0x4e9824),
    (0x8f, ArmKind::Std, 0x4e9cc4),
    (0x91, ArmKind::Std, 0x4e99a4),
    (0x92, ArmKind::Std, 0x4e99c4),
    (0x93, ArmKind::Std, 0x4e99f4),
    (0x94, ArmKind::Std, 0x4e9614),
    (0x95, ArmKind::Std, 0x4e9a14),
    (0x96, ArmKind::Std, 0x4e9a24),
    (0x97, ArmKind::Std, 0x4e92c4),
    (0x98, ArmKind::Std, 0x4e92c4),
    (0x9a, ArmKind::Std, 0x4e9a64),
    (0x9b, ArmKind::Std, 0x4e9a74),
    (0x9c, ArmKind::Std, 0x4e9a74),
    (0x9d, ArmKind::Std, 0x4e9a84),
    (0x9e, ArmKind::Std, 0x4e9a94),
    (0x9f, ArmKind::Std, 0x4e9aa4),
    (0xa0, ArmKind::Std, 0x4e9ab4),
    (0xa1, ArmKind::Std, 0x52465c),
    (0xa2, ArmKind::Std, 0x4e9344),
    (0xa3, ArmKind::Std, 0x4e9ad4),
    (0xa4, ArmKind::Std, 0x4e9ae4),
    (0xa5, ArmKind::Std, 0x4e9af4),
    (0xa6, ArmKind::Std, 0x4e95f4),
    (0xa7, ArmKind::Std, 0x4e9b04),
    (0xa8, ArmKind::Std, 0x4e9b14),
    (0xa9, ArmKind::Std, 0x4e9b24),
    (0xaa, ArmKind::Std, 0x4e9b34),
    (0xab, ArmKind::Std, 0x4e9b44),
    (0xad, ArmKind::Std, 0x4e9b54),
    (0xae, ArmKind::Std, 0x4e9b64),
    (0xaf, ArmKind::Std, 0x4e9b74),
    (0xb0, ArmKind::Std, 0x4e9b84),
    (0xb1, ArmKind::Std, 0x4e9b94),
    (0xb2, ArmKind::Std, 0x4e9ba4),
    (0xb3, ArmKind::Std, 0x4e9bb4),
    (0xb4, ArmKind::Alloc, 0x4e9f04),
    (0xb5, ArmKind::Alloc, 0x4e9f14),
    (0xb6, ArmKind::Std, 0x4e9bc4),
    (0xb7, ArmKind::Alloc, 0x4e9f24),
    (0xb8, ArmKind::Alloc, 0x4e9f34),
    (0xb9, ArmKind::Alloc, 0x4e9f44),
    (0xba, ArmKind::Std, 0x4e9bd4),
    (0xbb, ArmKind::Std, 0x4e9be4),
    (0xbc, ArmKind::Std, 0x4e9be4),
    (0xbd, ArmKind::Std, 0x4e9bf4),
    (0xbe, ArmKind::Std, 0x4e9c04),
    (0xbf, ArmKind::Alloc, 0x4e9f54),
    (0xc0, ArmKind::Alloc, 0x4e9f64),
    (0xc1, ArmKind::Alloc, 0x4e9f74),
    (0xc2, ArmKind::Alloc, 0x4e9f84),
    (0xc3, ArmKind::Std, 0x4e9c14),
    (0xc4, ArmKind::Std, 0x4e9c24),
    (0xc5, ArmKind::Std, 0x4e9c34),
    (0xc6, ArmKind::Std, 0x4e9c44),
    (0xc7, ArmKind::Alloc, 0x4e9f94),
    (0xc8, ArmKind::Alloc, 0x4e9fa4),
    (0xc9, ArmKind::Alloc, 0x4e9fb4),
    (0xca, ArmKind::Alloc, 0x4e9fc4),
    (0xcb, ArmKind::Alloc, 0x4e9fd4),
    (0xcc, ArmKind::Alloc, 0x4e9fe4),
    (0xcd, ArmKind::Alloc, 0x4e9ff4),
    (0xce, ArmKind::Alloc, 0x4ea004),
    (0xcf, ArmKind::Alloc, 0x4ea014),
    (0xd0, ArmKind::Alloc, 0x4ea024),
    (0xd1, ArmKind::Alloc, 0x4ea034),
    (0xd2, ArmKind::Alloc, 0x4ea044),
    (0xd3, ArmKind::Std, 0x4e9c74),
    (0xd4, ArmKind::Std, 0x4e9c94),
    (0xd5, ArmKind::Std, 0x4e9a34),
    (0xd6, ArmKind::Std, 0x4e9cb4),
    (0xd7, ArmKind::Std, 0x4e9cc4),
    (0xd8, ArmKind::Std, 0x4e9ce4),
    (0xd9, ArmKind::Std, 0x4e9cf4),
    (0xda, ArmKind::Std, 0x4e9d04),
    (0xdb, ArmKind::Std, 0x4e9d14),
    (0xdc, ArmKind::Std, 0x4e9d54),
    (0xde, ArmKind::Std, 0x4e9d64),
    (0xdf, ArmKind::Std, 0x4e9d84),
    (0xe0, ArmKind::Std, 0x4e9d94),
    (0xe1, ArmKind::Std, 0x4e9da4),
    (0xe2, ArmKind::Std, 0x4e9db4),
    (0xe3, ArmKind::Std, 0x4e9dc4),
    (0xe4, ArmKind::Std, 0x4e9dd4),
    (0xe5, ArmKind::Std, 0x4e9de4),
];

/// Look up the arm of `code` (binary search over [`ARMS`]).
pub fn arm(code: u32) -> Option<(ArmKind, u32)> {
    ARMS.binary_search_by_key(&code, |&(c, _, _)| c)
        .ok()
        .map(|i| (ARMS[i].1, ARMS[i].2))
}

/// Open the page for message `code` on `object`.
pub fn menu_message_dispatch<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
    object: u32,
    code: u32,
) {
    if let Some((kind, descriptor)) = arm(code) {
        let state = MenuState::load(m);
        match kind {
            ArmKind::Std => {
                if state.click_sound_enabled != 0 {
                    let request = m.read32(state.click_sound_request);
                    call_cdecl(guest, m, r, PLAY_CLICK, &[request]);
                }
                // The original tail-jumps into the push routine with its own argument slots rewritten in
                // place (the caller's `object` and `code` words), so the port does the same.
                let ui_state = m.read32(crate::menu_state::UI_STATE_OBJECT);
                m.write32(r.esp.wrapping_add(8), descriptor);
                m.write32(r.esp.wrapping_add(4), ui_state);
                r.tail_call(guest, PUSH_PAGE);
            }
            ArmKind::Alloc => {
                call_cdecl(
                    guest,
                    m,
                    r,
                    CREATE_OR_FIND_CHILD,
                    &[object, descriptor, CHILD_MODE],
                );
                let child = r.eax;
                MenuStateView.set_open_transition_flag(m, 1);
                call_cdecl(guest, m, r, OPEN_TRANSITION, &[object, child]);
            }
        }
        return;
    }
    let ui_state = m.read32(crate::menu_state::UI_STATE_OBJECT);
    match code {
        0x43 => call_cdecl(guest, m, r, 0x0019_6050, &[object]),
        0x90 => call_cdecl(guest, m, r, POP_PAGE, &[ui_state]),
        0x99 => call_cdecl(guest, m, r, 0x0003_89b0, &[object]),
        0xac => call_cdecl(guest, m, r, 0x001a_08b0, &[object]),
        0xdd => call_cdecl(guest, m, r, 0x001a_1490, &[object]),
        _ => {}
    }
}

/// Entry with the guest calling convention: `object` at `esp + 4`, `code` at `esp + 8`.
pub fn run_1af950<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, r: &mut ExtRegisters) {
    let object = m.read32(r.esp.wrapping_add(4));
    let code = m.read32(r.esp.wrapping_add(8));
    menu_message_dispatch(m, guest, r, object, code);
}

/// Hybrid hook for `c2r::run`: handles 0x1AF950 and reports whether it did.
pub fn try_run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) -> bool {
    if va != 0x001a_f950 {
        return false;
    }
    run_1af950(m, guest, r);
    true
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::{cell::RefCell, collections::BTreeMap, rc::Rc, vec::Vec};

    #[derive(Clone, Default)]
    struct Memory(Rc<RefCell<BTreeMap<u32, u8>>>);
    impl GuestMemory for Memory {
        fn read8(&self, a: u32) -> u8 {
            *self.0.borrow().get(&a).unwrap_or(&0)
        }
        fn write8(&mut self, a: u32, v: u8) {
            self.0.borrow_mut().insert(a, v);
        }
    }
    /// Records `(va, the three stack words at the callee's argument block)`; the child widget answer is 0x7777.
    struct Calls(Vec<(u32, [u32; 3])>, Memory);
    impl GuestCall for Calls {
        fn call(&mut self, va: u32, r: &mut ExtRegisters) {
            let w = |i: u32| self.1.read32(r.esp + 4 * i);
            self.0.push((va, [w(0), w(1), w(2)]));
            r.eax = if va == CREATE_OR_FIND_CHILD {
                0x7777
            } else {
                0
            };
        }
    }

    fn run_code(code: u32, click: bool) -> (Vec<(u32, [u32; 3])>, Memory) {
        let mut m = Memory::default();
        m.write32(crate::menu_state::CLICK_SOUND_ENABLED, u32::from(click));
        m.write32(crate::menu_state::CLICK_SOUND_REQUEST, 0x4800);
        m.write32(0x4800, 0x1234);
        m.write32(crate::menu_state::UI_STATE_OBJECT, 0x9000);
        let mut r = ExtRegisters {
            eax: 0,
            ecx: 0,
            edx: 0,
            esp: 0x7fff0,
            ebx: 0,
            ebp: 0,
            esi: 0,
            edi: 0,
            eflags: 0,
            fpu: crate::abi::Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0x27f,
            },
            xmm: Default::default(),
            xmm_hi: Default::default(),
        };
        let mut calls = Calls(Vec::new(), m.clone());
        menu_message_dispatch(&mut m, &mut calls, &mut r, 0x4000, code);
        assert_eq!(r.esp, 0x7fff0, "cdecl: the stack is balanced");
        (calls.0, m)
    }
    fn vas(calls: &[(u32, [u32; 3])]) -> Vec<u32> {
        calls.iter().map(|c| c.0).collect()
    }

    #[test]
    fn table_is_sorted_complete_and_distinct_from_the_helper_codes() {
        assert!(ARMS.windows(2).all(|w| w[0].0 < w[1].0));
        let helpers = [0x43, 0x90, 0x99, 0xac, 0xdd];
        assert_eq!(
            ARMS.len() + helpers.len(),
            0xe6,
            "every code 0..=0xe5 has an arm"
        );
        assert!(helpers.iter().all(|&c| arm(c).is_none()));
        assert!(ARMS.iter().all(|&(c, _, d)| c <= 0xe5 && d >= 0x4e_0000));
    }

    #[test]
    fn std_arm_clicks_only_when_enabled_then_pushes_the_page() {
        let (calls, _) = run_code(0, true);
        let (_, _, descriptor) = ARMS[0];
        assert_eq!(calls[0], (PLAY_CLICK, [0x1234, 0, 0]));
        assert_eq!(calls[1], (PUSH_PAGE, [0x9000, descriptor, 0]));
        assert_eq!(vas(&run_code(0, false).0), [PUSH_PAGE]);
    }

    #[test]
    fn alloc_arm_opens_child_and_sets_the_transition_flag() {
        let (calls, m) = run_code(0x18, true);
        assert_eq!(
            calls[0],
            (CREATE_OR_FIND_CHILD, [0x4000, arm(0x18).unwrap().1, 2])
        );
        assert_eq!(calls[1], (OPEN_TRANSITION, [0x4000, 0x7777, 0]));
        assert_eq!(calls.len(), 2);
        assert_eq!(m.read32(crate::menu_state::OPEN_TRANSITION_FLAG), 1);
    }

    #[test]
    fn helper_and_out_of_range_codes() {
        assert_eq!(run_code(0x43, true).0, [(0x196050, [0x4000, 0, 0])]);
        assert_eq!(run_code(0x90, true).0, [(POP_PAGE, [0x9000, 0, 0])]);
        assert_eq!(run_code(0x99, true).0, [(0x389b0, [0x4000, 0, 0])]);
        assert_eq!(run_code(0xac, true).0, [(0x1a08b0, [0x4000, 0, 0])]);
        assert_eq!(run_code(0xdd, true).0, [(0x1a1490, [0x4000, 0, 0])]);
        assert!(run_code(0xe6, true).0.is_empty());
        assert!(run_code(0xffff_ffff, true).0.is_empty());
    }
}
