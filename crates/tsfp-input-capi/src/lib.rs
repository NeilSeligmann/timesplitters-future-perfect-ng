// SPDX-License-Identifier: GPL-3.0-or-later
//! Small C ABI adapter for the safe XInput device model in `tsfp-input`.

use std::ffi::c_void;
use std::sync::{Mutex, OnceLock};

use tsfp_input::device_hle::{DeviceError, DeviceHle, GuestMemory, PadPolicy};

#[cfg(feature = "rust-input-host")]
pub use tsfp_input_host_wiring as host_wiring;
#[cfg(feature = "rust-input-host")]
pub use tsfp_input_hotkey_host as hotkey_host;
#[cfg(feature = "rust-input-host")]
pub use tsfp_input_mu as mu_device;
#[cfg(feature = "rust-input-host")]
pub use tsfp_input_route_engine as route_engine;
#[cfg(feature = "rust-input-host")]
pub use tsfp_input_route_spec as route_spec;

mod capture_replay;
mod devices;
mod dispatch;
mod host_source;
mod hotkey;
mod mu;
mod poll;
mod record;
mod script;

const OK: u32 = 0;
const INVALID_ADAPTER: u32 = 1;
const LOCK_POISONED: u32 = 2;
static DEVICE_HLE: OnceLock<Mutex<DeviceHle>> = OnceLock::new();

type GuestRead = unsafe extern "C" fn(*mut c_void, u32, *mut u8, usize) -> bool;
type GuestWrite = unsafe extern "C" fn(*mut c_void, u32, *const u8, usize) -> bool;

struct CGuestMemory {
    user: *mut c_void,
    read: GuestRead,
    write: GuestWrite,
}

impl GuestMemory for CGuestMemory {
    fn read(&self, address: u32, output: &mut [u8]) -> bool {
        // SAFETY: the caller supplies callbacks that honor the provided buffer and length.
        unsafe { (self.read)(self.user, address, output.as_mut_ptr(), output.len()) }
    }

    fn write(&mut self, address: u32, bytes: &[u8]) -> bool {
        // SAFETY: the caller supplies callbacks that only read the provided slice.
        unsafe { (self.write)(self.user, address, bytes.as_ptr(), bytes.len()) }
    }
}

impl CGuestMemory {
    fn new(
        user: *mut c_void,
        read: Option<GuestRead>,
        write: Option<GuestWrite>,
    ) -> Result<Self, u32> {
        match (read, write) {
            (Some(read), Some(write)) => Ok(Self { user, read, write }),
            _ => Err(INVALID_ADAPTER),
        }
    }
}

fn policy(connected_ports: u32, synthetic: bool, multiport: bool, removed: bool) -> PadPolicy {
    PadPolicy {
        connected_ports,
        synthetic_enabled: synthetic,
        multiport_enabled: multiport,
        removed,
    }
}

fn code(error: DeviceError) -> u32 {
    match error {
        DeviceError::SyntheticPadRequired => 10,
        DeviceError::PortZeroSyntheticRequired => 11,
        DeviceError::RepeatedInitialization => 12,
        DeviceError::WrongDeclarationCount => 13,
        DeviceError::DeclarationMemoryUnavailable => 14,
        DeviceError::DeclarationShapeNotMeasured => 15,
        DeviceError::UnknownDeviceType => 16,
        DeviceError::TableMemoryUnavailable => 17,
        DeviceError::DeviceTableNotCold => 18,
        DeviceError::InvalidMuMask => 19,
        DeviceError::StatusMemoryUnavailable => 20,
        DeviceError::DeclarationsAliasStatus => 21,
        DeviceError::OutputMemoryUnavailable => 22,
        DeviceError::OutputAliasesDeviceTable => 23,
        DeviceError::OutputsOverlap => 24,
        DeviceError::NotInitialized => 25,
    }
}

fn with_hle<T>(run: impl FnOnce(&mut DeviceHle) -> T) -> Result<T, u32> {
    DEVICE_HLE
        .get_or_init(|| Mutex::new(DeviceHle::default()))
        .lock()
        .map(|mut hle| run(&mut hle))
        .map_err(|_| LOCK_POISONED)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_device_hle_reset() -> u32 {
    with_hle(|hle| hle.reset()).map_or_else(|error| error, |_| OK)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_device_hle_set_mu_mask(
    mask: u32,
    user: *mut c_void,
    read: Option<GuestRead>,
    write: Option<GuestWrite>,
) -> u32 {
    let mut memory = match CGuestMemory::new(user, read, write) {
        Ok(memory) => memory,
        Err(error) => return error,
    };
    with_hle(|hle| hle.set_mu_mask(&mut memory, mask))
        .map_or_else(|error| error, |result| result.map_or_else(code, |_| OK))
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_device_hle_initialize(
    count: u32,
    declarations: u32,
    connected_ports: u32,
    synthetic: bool,
    multiport: bool,
    removed: bool,
    user: *mut c_void,
    read: Option<GuestRead>,
    write: Option<GuestWrite>,
    insertions_out: *mut u32,
) -> u32 {
    if insertions_out.is_null() {
        return INVALID_ADAPTER;
    }
    let mut memory = match CGuestMemory::new(user, read, write) {
        Ok(memory) => memory,
        Err(error) => return error,
    };
    let result = with_hle(|hle| {
        hle.initialize(
            &mut memory,
            count,
            declarations,
            policy(connected_ports, synthetic, multiport, removed),
        )
    });
    match result {
        Err(error) => error,
        Ok(Err(error)) => code(error),
        Ok(Ok(initialization)) => {
            // SAFETY: the caller provided a non-null pointer to one writable u32.
            unsafe { insertions_out.write(initialization.synthetic_insertions) };
            OK
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_device_hle_get(
    device_type: u32,
    connected_ports: u32,
    synthetic: bool,
    multiport: bool,
    removed: bool,
    user: *mut c_void,
    read: Option<GuestRead>,
    write: Option<GuestWrite>,
    value_out: *mut u32,
) -> u32 {
    if value_out.is_null() {
        return INVALID_ADAPTER;
    }
    let mut memory = match CGuestMemory::new(user, read, write) {
        Ok(memory) => memory,
        Err(error) => return error,
    };
    let result = with_hle(|hle| {
        hle.get(
            &mut memory,
            device_type,
            policy(connected_ports, synthetic, multiport, removed),
        )
    });
    write_result(result, value_out)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_device_hle_peek(
    device_type: u32,
    previous: u32,
    reconnected: u32,
    connected_ports: u32,
    synthetic: bool,
    multiport: bool,
    removed: bool,
    user: *mut c_void,
    read: Option<GuestRead>,
    write: Option<GuestWrite>,
    value_out: *mut u32,
) -> u32 {
    if value_out.is_null() {
        return INVALID_ADAPTER;
    }
    let mut memory = match CGuestMemory::new(user, read, write) {
        Ok(memory) => memory,
        Err(error) => return error,
    };
    let result = with_hle(|hle| {
        hle.peek(
            &mut memory,
            device_type,
            previous,
            reconnected,
            policy(connected_ports, synthetic, multiport, removed),
        )
    });
    write_result(result, value_out)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_device_hle_changes(
    device_type: u32,
    insertions: u32,
    removals: u32,
    connected_ports: u32,
    synthetic: bool,
    multiport: bool,
    removed: bool,
    user: *mut c_void,
    read: Option<GuestRead>,
    write: Option<GuestWrite>,
    changed_out: *mut bool,
) -> u32 {
    if changed_out.is_null() {
        return INVALID_ADAPTER;
    }
    let mut memory = match CGuestMemory::new(user, read, write) {
        Ok(memory) => memory,
        Err(error) => return error,
    };
    let result = with_hle(|hle| {
        hle.changes(
            &mut memory,
            device_type,
            insertions,
            removals,
            policy(connected_ports, synthetic, multiport, removed),
        )
    });
    write_result(result, changed_out)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_device_hle_enumeration_status(
    connected_ports: u32,
    synthetic: bool,
    multiport: bool,
    removed: bool,
    user: *mut c_void,
    read: Option<GuestRead>,
    write: Option<GuestWrite>,
    status_out: *mut bool,
) -> u32 {
    if status_out.is_null() {
        return INVALID_ADAPTER;
    }
    let memory = match CGuestMemory::new(user, read, write) {
        Ok(memory) => memory,
        Err(error) => return error,
    };
    let result = with_hle(|hle| {
        hle.enumeration_status(
            &memory,
            policy(connected_ports, synthetic, multiport, removed),
        )
    });
    write_result(result, status_out)
}

fn write_result<T: Copy>(result: Result<Result<T, DeviceError>, u32>, output: *mut T) -> u32 {
    match result {
        Err(error) => error,
        Ok(Err(error)) => code(error),
        Ok(Ok(value)) => {
            // SAFETY: every export checks its non-null output pointer before calling here.
            unsafe { output.write(value) };
            OK
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: u32 = 0x0040_0000;
    const SIZE: usize = 0x0040_0000;

    struct Guest(Vec<u8>);

    impl Guest {
        fn write_at(&mut self, address: u32, bytes: &[u8]) {
            let start = (address - BASE) as usize;
            self.0[start..start + bytes.len()].copy_from_slice(bytes);
        }

        fn words(&self, address: u32) -> [u32; 3] {
            let start = (address - BASE) as usize;
            std::array::from_fn(|index| {
                let at = start + index * 4;
                u32::from_le_bytes(self.0[at..at + 4].try_into().unwrap())
            })
        }
    }

    unsafe extern "C" fn read_guest(
        user: *mut c_void,
        address: u32,
        output: *mut u8,
        length: usize,
    ) -> bool {
        if user.is_null() || output.is_null() || address < BASE {
            return false;
        }
        let start = (address - BASE) as usize;
        if start.checked_add(length).is_none_or(|end| end > SIZE) {
            return false;
        }
        // SAFETY: this callback receives the test's live Guest pointer and an output buffer
        // supplied by the adapter for exactly `length` bytes.
        unsafe {
            let guest = &*user.cast::<Guest>();
            std::ptr::copy_nonoverlapping(guest.0.as_ptr().add(start), output, length);
        }
        true
    }

    unsafe extern "C" fn write_guest(
        user: *mut c_void,
        address: u32,
        input: *const u8,
        length: usize,
    ) -> bool {
        if user.is_null() || input.is_null() || address < BASE {
            return false;
        }
        let start = (address - BASE) as usize;
        if start.checked_add(length).is_none_or(|end| end > SIZE) {
            return false;
        }
        // SAFETY: this callback receives the test's live Guest pointer and an input buffer
        // supplied by the adapter for exactly `length` bytes.
        unsafe {
            let guest = &mut *user.cast::<Guest>();
            std::ptr::copy_nonoverlapping(input, guest.0.as_mut_ptr().add(start), length);
        }
        true
    }

    #[test]
    fn ffi_calls_preserve_guest_memory_masks_and_errors() {
        let mut guest = Guest(vec![0; SIZE]);
        let declarations = [
            0x0046_C6E0u32,
            8,
            0x0046_C8A0,
            4,
            0x0046_C894,
            4,
            0x0046_C75C,
            4,
        ];
        guest.write_at(0x0050_0000, &bytemuck_words(&declarations));
        let user = (&mut guest as *mut Guest).cast::<c_void>();
        assert_eq!(tsfp_input_device_hle_reset(), OK);

        let mut insertions = u32::MAX;
        assert_eq!(
            tsfp_input_device_hle_initialize(
                4,
                0x0050_0000,
                0,
                false,
                false,
                false,
                user,
                Some(read_guest),
                Some(write_guest),
                &mut insertions,
            ),
            OK
        );
        assert_eq!(insertions, 0);

        guest.write_at(0x0046_C75C, &bytemuck_words(&[2, 2, 0]));
        let mut changed = false;
        assert_eq!(
            tsfp_input_device_hle_changes(
                0x0046_C75C,
                0x0051_0000,
                0x0051_0004,
                0,
                false,
                false,
                false,
                user,
                Some(read_guest),
                Some(write_guest),
                &mut changed,
            ),
            OK
        );
        assert!(changed);
        assert_eq!(
            u32::from_le_bytes(guest.0[0x0011_0000..0x0011_0004].try_into().unwrap()),
            2
        );
        assert_eq!(guest.words(0x0046_C75C), [2, 0, 2]);

        let mut value = u32::MAX;
        assert_eq!(
            tsfp_input_device_hle_get(
                0x0046_C75C,
                0,
                false,
                false,
                false,
                user,
                Some(read_guest),
                Some(write_guest),
                &mut value,
            ),
            OK
        );
        assert_eq!(value, 2);
        assert_eq!(guest.words(0x0046_C75C), [2, 0, 2]);

        assert_eq!(
            tsfp_input_device_hle_get(
                0x1234,
                0,
                false,
                false,
                false,
                user,
                Some(read_guest),
                Some(write_guest),
                &mut value,
            ),
            16
        );
    }

    fn bytemuck_words(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|word| word.to_le_bytes()).collect()
    }
}
