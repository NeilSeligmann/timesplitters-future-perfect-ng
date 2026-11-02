// SPDX-License-Identifier: GPL-3.0-or-later
//! Private bounded memory for structural contract tests, independent of the C oracle.
use tsfp_kernel_services::GuestMemory;

#[derive(Clone)]
pub struct Memory(pub Vec<u8>);
impl GuestMemory for Memory {
    fn readable(&self, address: u32, length: usize) -> bool {
        address != 0
            && (address as usize)
                .checked_add(length)
                .is_some_and(|end| end <= self.0.len())
    }
    fn read(&self, address: u32, out: &mut [u8]) -> bool {
        if !self.readable(address, out.len()) {
            return false;
        }
        out.copy_from_slice(&self.0[address as usize..address as usize + out.len()]);
        true
    }
    fn write(&mut self, address: u32, bytes: &[u8]) -> bool {
        if !self.readable(address, bytes.len()) {
            return false;
        }
        self.0[address as usize..address as usize + bytes.len()].copy_from_slice(bytes);
        true
    }
}
