// SPDX-License-Identifier: GPL-3.0-or-later
//! FABRICATED low-8-MiB fixture for typed-state round-trip and write-bound tests.
use tsfp_state::GuestMemory;

pub struct Image(pub Vec<u8>);

impl GuestMemory for Image {
    fn read8(&self, address: u32) -> u8 {
        self.0[address as usize]
    }

    fn write8(&mut self, address: u32, value: u8) {
        self.0[address as usize] = value;
    }
}

pub fn image() -> Image {
    Image((0..0x80_0000usize).map(|i| (i * 7 + 3) as u8).collect())
}
