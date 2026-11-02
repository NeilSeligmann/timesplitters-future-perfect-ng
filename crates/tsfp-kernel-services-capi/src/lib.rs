// SPDX-License-Identifier: GPL-3.0-or-later
//! Narrow unsafe ABI adapter; readable service logic remains in the safe library.
//! Caller serializes setup and dispatch and keeps callback buffers alive for each call.
use std::{
    collections::BTreeMap,
    sync::{LazyLock, Mutex},
};
use tsfp_kernel_services::{
    AvPack, GuestMemory, KernelServices, RustKernelServices,
    settings::{Settings, UnknownPolicy},
};
type Read = unsafe extern "C" fn(u32, *mut u8, usize) -> bool;
type Write = unsafe extern "C" fn(u32, *const u8, usize) -> bool;
type Readable = unsafe extern "C" fn(u32, usize) -> bool;
static SERVICES: LazyLock<Mutex<RustKernelServices>> =
    LazyLock::new(|| Mutex::new(RustKernelServices::default()));

struct Memory {
    read: Read,
    write: Write,
    readable: Readable,
    // Original and final bytes, including successful prefixes of failed writes.
    changed: BTreeMap<u32, (u8, u8)>,
    reversible: bool,
}
impl GuestMemory for Memory {
    fn readable(&self, address: u32, length: usize) -> bool {
        unsafe { (self.readable)(address, length) }
    }
    fn read(&self, address: u32, output: &mut [u8]) -> bool {
        unsafe { (self.read)(address, output.as_mut_ptr(), output.len()) }
    }
    fn write(&mut self, address: u32, input: &[u8]) -> bool {
        // Byte writes make successful prefixes observable even when the host copy fails.
        for (offset, &byte) in input.iter().enumerate() {
            let Some(at) = u32::try_from(offset)
                .ok()
                .and_then(|n| address.checked_add(n))
                .filter(|&n| n != 0)
            else {
                return false;
            };
            let mut old = [0];
            if !self.read(at, &mut old) {
                self.reversible = false;
                return false;
            }
            if !unsafe { (self.write)(at, &byte, 1) } {
                return false;
            }
            self.changed
                .entry(at)
                .and_modify(|v| v.1 = byte)
                .or_insert((old[0], byte));
        }
        true
    }
}
pub struct Pending {
    restored: bool,
    eax: u32,
    changed: BTreeMap<u32, (u8, u8)>,
    reversible: bool,
}

/// Decode only the supplied argument prefix. All guest access uses guarded C callbacks.
/// Returns owned replay evidence; the C oracle runs after the Rust mutex is released.
/// # Safety
/// args must point to count aligned readable u32 words; callbacks obey GuestMemory and remain valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_services_prepare(
    ordinal: u32,
    args: *const u32,
    count: usize,
    thread: u64,
    read: Read,
    write: Write,
    readable: Readable,
) -> *mut Pending {
    if count > 257 || (count != 0 && args.is_null()) {
        return std::ptr::null_mut();
    }
    let args = if count == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(args, count) }
    };
    let mut memory = Memory {
        read,
        write,
        readable,
        changed: BTreeMap::new(),
        reversible: true,
    };
    let answer = SERVICES
        .lock()
        .unwrap()
        .call(ordinal, args, thread, &mut memory);
    let Some(answer) = answer else {
        return std::ptr::null_mut();
    };
    let mut restored = true;
    for (&at, &(old, _)) in &memory.changed {
        if !unsafe { write(at, &old, 1) } {
            restored = false;
        }
    }
    Box::into_raw(Box::new(Pending {
        restored,
        eax: answer.eax,
        changed: memory.changed,
        reversible: memory.reversible,
    }))
}

/// Consume evidence once. Compare final aliased bytes and EAX, then publish Rust only
/// on agreement. C has already delivered all diagnostic, terminal and display effects.
/// # Safety
/// pending must be null or an unconsumed prepare result; mismatch is writable and callbacks remain valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_services_finish(
    pending: *mut Pending,
    oracle: u32,
    selected: bool,
    read: Read,
    write: Write,
    mismatch: *mut bool,
) -> u32 {
    if pending.is_null() {
        unsafe {
            *mismatch = true;
        }
        return oracle;
    }
    let pending = unsafe { Box::from_raw(pending) };
    let mut agrees = pending.reversible && pending.eax == oracle;
    for (&at, &(_, expected)) in &pending.changed {
        let mut actual = 0;
        agrees &= unsafe { read(at, &mut actual, 1) } && actual == expected;
    }
    unsafe {
        *mismatch = !agrees;
    }
    if !agrees || !selected {
        return oracle;
    }
    // The selected output is Rust's output; never write Rust mismatches over C.
    for (&at, &(_, expected)) in &pending.changed {
        if !unsafe { write(at, &expected, 1) } {
            unsafe {
                *mismatch = true;
            }
            return oracle;
        }
    }
    pending.eax
}

/// Mirror a C subsystem reset, not the entire library (AV wide claim survives).
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_services_reset(ordinal: u32) {
    let mut s = SERVICES.lock().unwrap();
    match ordinal {
        24 => s.settings = Settings::default(),
        47 => s.hal = Default::default(),
        335 => s.crypto = Default::default(),
        1 => s.av.reset(),
        139 => s.fp = Default::default(),
        8 => {
            s.debug_prints = 0;
            s.debug_refused = 0;
        }
        _ => {}
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_services_policy(fail: bool) {
    SERVICES.lock().unwrap().settings.policy = if fail {
        UnknownPolicy::Fail
    } else {
        UnknownPolicy::FabricatedZeros
    };
}
/// # Safety
/// bytes must be readable for count bytes when nonzero; setup is serialized with dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_services_setting(index: u32, bytes: *const u8, count: usize) -> bool {
    if count > 256 || (count != 0 && bytes.is_null()) {
        return false;
    }
    let bytes = if count == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(bytes, count) }
    };
    SERVICES.lock().unwrap().settings.set(index, bytes)
}
/// # Safety
/// bytes must hold 256 readable bytes; a nonnull key holds 16 readable bytes; setup is serialized.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_services_eeprom(bytes: *const u8, key: *const u8) -> bool {
    if bytes.is_null() {
        return false;
    }
    let bytes = unsafe { &*bytes.cast::<[u8; 256]>() };
    let key = if key.is_null() {
        None
    } else {
        Some(unsafe { &*key.cast::<[u8; 16]>() })
    };
    SERVICES.lock().unwrap().settings.set_eeprom(bytes, key)
}
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_services_av(pack: u32, region: u32) -> bool {
    let pack = match pack {
        0 => AvPack::Composite,
        1 => AvPack::Svideo,
        2 => AvPack::Hdtv,
        _ => return false,
    };
    SERVICES.lock().unwrap().av.configure(pack, region)
}
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_services_wide(wide: bool) {
    SERVICES.lock().unwrap().av.widescreen = wide;
}

/// Restore failure is terminal: the C oracle must not observe Rust-modified inputs.
/// # Safety
/// pending must be null or an unconsumed prepare result.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_services_restored(pending: *const Pending) -> bool {
    !pending.is_null() && unsafe { (*pending).restored }
}
/// # Safety
/// pending must be null or an unconsumed prepare result, consumed exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_services_discard(pending: *mut Pending) {
    if !pending.is_null() {
        drop(unsafe { Box::from_raw(pending) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Arena {
        bytes: [u8; 1024],
        writable: usize,
        writes: usize,
        reject_write: Option<usize>,
    }
    static ARENA: Mutex<Arena> = Mutex::new(Arena {
        bytes: [0; 1024],
        writable: 1024,
        writes: 0,
        reject_write: None,
    });
    unsafe extern "C" fn readable(address: u32, count: usize) -> bool {
        address != 0
            && (address as usize)
                .checked_add(count)
                .is_some_and(|end| end <= 1024)
    }
    unsafe extern "C" fn read(address: u32, out: *mut u8, count: usize) -> bool {
        if !unsafe { readable(address, count) } {
            return false;
        }
        let arena = ARENA.lock().unwrap();
        unsafe {
            std::ptr::copy_nonoverlapping(arena.bytes.as_ptr().add(address as usize), out, count);
        }
        true
    }
    unsafe extern "C" fn write(address: u32, input: *const u8, count: usize) -> bool {
        let mut arena = ARENA.lock().unwrap();
        arena.writes += 1;
        if arena.reject_write == Some(arena.writes)
            || address == 0
            || !(address as usize)
                .checked_add(count)
                .is_some_and(|end| end <= arena.writable)
        {
            return false;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(
                input,
                arena.bytes.as_mut_ptr().add(address as usize),
                count,
            );
        }
        true
    }
    #[test]
    fn guarded_replay_abi_and_restore_failure() {
        tsfp_services_reset(335);
        {
            let mut arena = ARENA.lock().unwrap();
            for (i, byte) in arena.bytes[100..116].iter_mut().enumerate() {
                *byte = i as u8;
            }
        }
        let original = ARENA.lock().unwrap().bytes;
        let args = [100, 16];
        let pending =
            unsafe { tsfp_services_prepare(346, args.as_ptr(), 2, 1, read, write, readable) };
        assert!(unsafe { tsfp_services_restored(pending) });
        assert_eq!(
            ARENA.lock().unwrap().bytes,
            original,
            "C sees original inputs"
        );
        // A separately computed parity oracle supplies the expected final bytes.
        for byte in &mut ARENA.lock().unwrap().bytes[100..116] {
            let high = *byte & 0xfe;
            *byte = high | u8::from(high.count_ones() % 2 == 0);
        }
        let mut mismatch = true;
        assert_eq!(
            unsafe { tsfp_services_finish(pending, 0, true, read, write, &mut mismatch) },
            0
        );
        assert!(!mismatch);
        assert!(
            ARENA.lock().unwrap().bytes[100..116]
                .iter()
                .all(|byte| byte.count_ones() % 2 == 1)
        );

        tsfp_services_reset(24);
        {
            let mut arena = ARENA.lock().unwrap();
            arena.bytes.fill(0xaa);
            arena.writable = 106;
            arena.writes = 0;
        }
        let args = [0x7654, 0, 100, 12, 0];
        let pending =
            unsafe { tsfp_services_prepare(24, args.as_ptr(), 5, 1, read, write, readable) };
        assert!(unsafe { tsfp_services_restored(pending) });
        assert!(ARENA.lock().unwrap().bytes.iter().all(|&byte| byte == 0xaa));
        ARENA.lock().unwrap().bytes[100..106].fill(0); // C's successful prefix.
        assert_eq!(
            unsafe { tsfp_services_finish(pending, 0xc000000d, true, read, write, &mut mismatch) },
            0xc000000d
        );
        assert!(!mismatch);
        assert_eq!(ARENA.lock().unwrap().bytes[106], 0xaa);

        {
            let mut arena = ARENA.lock().unwrap();
            arena.writable = 1024;
            arena.writes = 0;
            arena.reject_write = Some(17);
        }
        let args = [100, 16];
        let pending =
            unsafe { tsfp_services_prepare(346, args.as_ptr(), 2, 1, read, write, readable) };
        assert!(
            !unsafe { tsfp_services_restored(pending) },
            "failed rollback forbids running C"
        );
        unsafe {
            tsfp_services_discard(pending);
        }
        assert!(
            unsafe { tsfp_services_prepare(346, std::ptr::null(), 258, 1, read, write, readable) }
                .is_null()
        );
    }
}
