// SPDX-License-Identifier: GPL-3.0-or-later
//! T3975 slice 7: the memory-unit device of `src/input/mu_device.c` and `src/input/mu_startup.c` in Rust. The slot
//! table (images, paths, FATX volumes, guest device ids, eject flags) is Rust data behind the same `mu_*` C ABI.
//! FATX itself (`fatx_*`), the file-system namespace (`tsfp_rfio_*`) and the guest accessors stay behind `sys`.
//! `TSFP_RUST_INPUT_HLE=0` builds keep the C units as the baseline. Contract: docs/t3975-input-mu-rust.md.
// The C modules only exist in the host executable, so unit tests (no C) build the pure model alone.
#[cfg(not(test))]
mod device;
#[cfg(not(test))]
mod startup;
#[cfg(not(test))]
mod sys;

/// Win32 errors `XMountMUA` / `XUnmountMU` return.
pub mod errors {
    pub const INVALID_DRIVE: u32 = 0x0F;
    pub const ALREADY_ASSIGNED: u32 = 0x55;
    pub const DEVICE_NOT_CONNECTED: u32 = 0x48F;
    pub const UNRECOGNIZED_VOLUME: u32 = 0x3ED;
}

/// Drive letter of a port/slot pair (original index port*2+slot), `0` outside the 8-entry table.
pub fn drive_letter(port: u32, slot: u32) -> u8 {
    tsfp_input_devices::mu::index_of(port, slot).map_or(0, |index| 0x46 + index as u8)
}

/// Bit of the presence mask `xinput_devices_set_mu_mask` takes for a table index (port + 16 * slot).
pub fn presence_bit(index: usize) -> u32 {
    let (port, slot) = (index / 2, index % 2);
    1 << (port + 16 * slot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_follow_the_original_table() {
        assert_eq!(drive_letter(0, 0), b'F');
        assert_eq!(drive_letter(1, 0), b'H');
        assert_eq!(drive_letter(3, 1), b'M');
        assert_eq!(drive_letter(4, 0), 0);
        assert_eq!(drive_letter(0, 8), 0);
    }

    #[test]
    fn presence_bits_split_ports_and_slots() {
        assert_eq!(presence_bit(0), 1);
        assert_eq!(presence_bit(1), 1 << 16);
        assert_eq!(presence_bit(7), 1 << 19);
    }
}
