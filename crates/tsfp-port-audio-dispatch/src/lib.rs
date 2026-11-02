// SPDX-License-Identifier: GPL-3.0-or-later
//! Nested audio adapter routing, kept below the frozen `tsfp-ports` facade.
// portreg: replaces=tsfp-ports/audio
// portreg: replaces=tsfp-ports/audio_remainder
#![no_std]

use tsfp_port_runtime::abi::{ExtRegisters, GuestCall};

/// Audio roots currently admitted by the opt-in audio adapter group.
pub const ROOTS: [u32; 8] = [
    0x298b0, 0x27c20, 0x26d20, 0x164bf0, 0x165390, 0x27fa0, 0x29e40, 0x29e60,
];

static REPLAY_CAPTURED_CALLS: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// Set by the offline host while a captured function is executing through the lane dispatcher.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_port_audio_dispatch_set_capture_replay(enabled: i32) {
    REPLAY_CAPTURED_CALLS.store(enabled != 0, core::sync::atomic::Ordering::Relaxed);
}

#[inline]
fn replay_captured_calls() -> bool {
    REPLAY_CAPTURED_CALLS.load(core::sync::atomic::Ordering::Relaxed)
}

/// GuestCall decorator that gives nested calls the same Rust audio adapters as direct roots.
pub struct GuestRouter<'a, C: GuestCall> {
    guest: &'a mut C,
    route: fn(u32, &mut C, &mut ExtRegisters) -> bool,
}

impl<'a, C: GuestCall> GuestRouter<'a, C> {
    pub fn new(guest: &'a mut C, route: fn(u32, &mut C, &mut ExtRegisters) -> bool) -> Self {
        Self { guest, route }
    }
}

impl<C: GuestCall> GuestCall for GuestRouter<'_, C> {
    fn seh_ebp(&self) -> Option<u32> {
        self.guest.seh_ebp()
    }

    fn mmx_read(&self, index: usize) -> u64 {
        self.guest.mmx_read(index)
    }

    fn mmx_write(&mut self, index: usize, value: u64) {
        self.guest.mmx_write(index, value)
    }

    fn frame_wait_preflight(&mut self, getter: u32, return_address: u32) {
        self.guest.frame_wait_preflight(getter, return_address)
    }

    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        // T3742's offline comparator owns nested calls and replays their captured outcomes.
        // Preserve that call sequence; the nested Rust adapters are for normal runtime only.
        if replay_captured_calls() || !(self.route)(va, self.guest, regs) {
            self.guest.call(va, regs);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;

    struct Guest(Cell<u32>);
    impl GuestCall for Guest {
        fn call(&mut self, _va: u32, _regs: &mut ExtRegisters) {
            self.0.set(self.0.get() + 1);
        }
    }

    fn route(_va: u32, _guest: &mut Guest, _regs: &mut ExtRegisters) -> bool {
        true
    }

    #[test]
    fn captured_replay_uses_guest_call_instead_of_nested_adapter() {
        REPLAY_CAPTURED_CALLS.store(true, core::sync::atomic::Ordering::Relaxed);
        let mut guest = Guest(Cell::new(0));
        let mut router = GuestRouter::new(&mut guest, route);
        let mut regs = ExtRegisters {
            eax: 0,
            ecx: 0,
            edx: 0,
            esp: 0,
            ebx: 0,
            ebp: 0,
            esi: 0,
            edi: 0,
            eflags: 0,
            fpu: tsfp_port_runtime::abi::Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0,
            },
            xmm: Default::default(),
            xmm_hi: Default::default(),
        };
        router.call(0x26bb0, &mut regs);
        assert_eq!(router.guest.0.get(), 1);
        REPLAY_CAPTURED_CALLS.store(false, core::sync::atomic::Ordering::Relaxed);
    }
}
