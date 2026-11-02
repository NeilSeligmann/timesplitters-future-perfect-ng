// SPDX-License-Identifier: GPL-3.0-or-later
//! T3610: the upward call of `leaf_wire::set_player_byte` into the weapon-hands runtime, as a registered hook.
//!
//! `leaf_wire` (this crate) must tell the weapon-hands owner (`tsfp-ports::weapon_hands_runtime`, 4k lines that depend on
//! the whole facade) when the guest writes an "equipped" byte. A direct call would make this crate depend on the facade,
//! which is exactly the cycle that kept the C2R batches in it. The facade registers the two entry points the first time it
//! is configured (`weapon_hands_runtime::configure`); until then `enabled()` is false, which is what the facade flag
//! itself reports before the first `configure`, so behaviour is unchanged. `enabled` stays a call into the facade, so the
//! per-thread test switch of `weapon_hands_runtime` keeps working.
use crate::memory::GuestMemory;
use core::sync::atomic::{AtomicPtr, Ordering};

/// Entry points of the weapon-hands runtime that the wiring layer needs.
pub struct WeaponHandsHook {
    /// `weapon_hands_runtime::enabled`.
    pub enabled: fn() -> bool,
    /// `weapon_hands_runtime::mark_equipped_once` over a type-erased guest memory.
    pub mark_equipped_once: fn(&mut dyn GuestMemory, u32, usize) -> bool,
}

static HOOK: AtomicPtr<WeaponHandsHook> = AtomicPtr::new(core::ptr::null_mut());

/// Registers the runtime (idempotent, the hook table is a `'static` of the facade).
pub fn register(hook: &'static WeaponHandsHook) {
    HOOK.store(
        hook as *const WeaponHandsHook as *mut WeaponHandsHook,
        Ordering::Release,
    );
}

fn hook() -> Option<&'static WeaponHandsHook> {
    // SAFETY: only `register` stores here, always a `&'static WeaponHandsHook`.
    unsafe { HOOK.load(Ordering::Acquire).as_ref() }
}

/// False until the facade registered the runtime.
pub fn enabled() -> bool {
    hook().is_some_and(|hook| (hook.enabled)())
}

/// `weapon_hands_runtime::mark_equipped_once`; false when no runtime is registered.
pub fn mark_equipped_once<M: GuestMemory>(m: &mut M, ext: u32, entry: usize) -> bool {
    match hook() {
        Some(hook) => (hook.mark_equipped_once)(m, ext, entry),
        None => false,
    }
}

/// `&mut dyn GuestMemory` as a sized `GuestMemory`: every trait method is forwarded, so an override of the concrete memory
/// (`read_bytes`, `as_slice`, `is_readable`) is honoured exactly as for the original generic call.
pub struct ErasedMemory<'a>(pub &'a mut dyn GuestMemory);

impl GuestMemory for ErasedMemory<'_> {
    fn is_readable(&self, address: u32, len: u32) -> bool {
        self.0.is_readable(address, len)
    }
    fn read8(&self, address: u32) -> u8 {
        self.0.read8(address)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.write8(address, value)
    }
    fn read16(&self, address: u32) -> u16 {
        self.0.read16(address)
    }
    fn read32(&self, address: u32) -> u32 {
        self.0.read32(address)
    }
    fn read_bytes(&self, address: u32, out: &mut [u8]) {
        self.0.read_bytes(address, out)
    }
    fn as_slice(&self, address: u32, len: u32) -> Option<&[u8]> {
        self.0.as_slice(address, len)
    }
    fn write32(&mut self, address: u32, value: u32) {
        self.0.write32(address, value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Ram([u8; 16]);
    impl GuestMemory for Ram {
        fn read8(&self, address: u32) -> u8 {
            self.0[address as usize]
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0[address as usize] = value;
        }
    }

    #[test]
    fn unregistered_hook_is_off() {
        // Another test thread may have registered the static; only the entry point contract is checked here.
        let mut ram = Ram([0; 16]);
        if !enabled() {
            assert!(!mark_equipped_once(&mut ram, 0, 0));
        }
    }

    #[test]
    fn erased_memory_forwards_every_access() {
        let mut ram = Ram([0; 16]);
        let mut erased = ErasedMemory(&mut ram);
        erased.write32(4, 0x1122_3344);
        assert_eq!(erased.read32(4), 0x1122_3344);
        assert_eq!(erased.read16(5), 0x2233);
        assert!(erased.is_readable(0, 16));
        let mut out = [0u8; 2];
        erased.read_bytes(6, &mut out);
        assert_eq!(out, [0x22, 0x11]);
        assert!(erased.as_slice(0, 4).is_none());
    }
}
