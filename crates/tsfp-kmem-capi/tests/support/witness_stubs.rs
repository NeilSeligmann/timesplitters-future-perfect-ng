// SPDX-License-Identifier: GPL-3.0-or-later
//! Stand-ins for the host's direct-write witness symbols (guest_mem.c, T3906) so integration tests link standalone;
//! recording is always off here.
#[unsafe(no_mangle)]
pub static tsfp_hostmem_direct_active: u8 = 0;
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_hostmem_direct_prepare() -> u64 {
    0
}
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_hostmem_direct_write(
    _token: u64,
    _address: u32,
    _bytes: *const u8,
    _length: usize,
) {
}
