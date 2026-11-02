//! Guest word reads for live DirectSound stack frames on the Rust-host path.
use tsfp_audio::adapter::GuestMemory;

use crate::effects_guest_copy_ffi::{
    tsfp_effects_guest_copy_read_rust, tsfp_effects_guest_copy_write_rust,
};

pub(crate) struct FrameGuestMemory(i32);

impl FrameGuestMemory {
    pub(crate) const fn new(host_pid: i32) -> Self {
        Self(host_pid)
    }

    pub(crate) fn write_bytes(&mut self, address: u32, bytes: &[u8]) -> bool {
        let Some(_length) = u32::try_from(bytes.len()).ok() else {
            return false;
        };
        if bytes.is_empty() {
            return true;
        }
        // SAFETY: the helper validates both spans and uses fault-safe process_vm_writev.
        unsafe { tsfp_effects_guest_copy_write_rust(self.0, bytes.as_ptr(), address, bytes.len()) }
    }
}

impl GuestMemory for FrameGuestMemory {
    fn read_u32(&self, address: u32) -> Option<u32> {
        let mut bytes = [0; 4];
        // SAFETY: the helper validates the guest span and uses fault-safe process_vm_readv.
        let copied = unsafe {
            tsfp_effects_guest_copy_read_rust(self.0, address, bytes.as_mut_ptr(), bytes.len())
        };
        copied.then(|| u32::from_le_bytes(bytes))
    }

    fn write_u32(&mut self, address: u32, value: u32) -> bool {
        let bytes = value.to_le_bytes();
        // SAFETY: the helper validates both spans and uses fault-safe process_vm_writev.
        unsafe { tsfp_effects_guest_copy_write_rust(self.0, bytes.as_ptr(), address, bytes.len()) }
    }

    fn read_bytes(&self, address: u32, bytes: usize) -> Option<Vec<u8>> {
        let length = u32::try_from(bytes).ok()?;
        if length == 0 {
            return Some(Vec::new());
        }
        let mut output = vec![0; bytes];
        // SAFETY: the helper validates the guest span and uses fault-safe process_vm_readv.
        let copied = unsafe {
            tsfp_effects_guest_copy_read_rust(self.0, address, output.as_mut_ptr(), bytes)
        };
        copied.then_some(output)
    }

    fn range_readable(&self, _: u32, _: u32) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_an_identity_mapped_guest_word_with_fault_safe_copy() {
        let (page, address) = crate::test_guest_memory::map_guest_page();
        // SAFETY: the test owns a writable page and writes one aligned word.
        unsafe { page.cast::<u32>().write(0xA1B2_C3D4) };
        let memory = FrameGuestMemory::new(std::process::id() as i32);
        assert_eq!(memory.read_u32(address), Some(0xA1B2_C3D4));
        crate::test_guest_memory::unmap_guest_page(page);
    }

    #[test]
    fn writes_an_identity_mapped_guest_word_with_fault_safe_copy() {
        let (page, address) = crate::test_guest_memory::map_guest_page();
        let mut memory = FrameGuestMemory::new(std::process::id() as i32);
        assert!(memory.write_u32(address, 0xA1B2_C3D4));
        assert_eq!(memory.read_u32(address), Some(0xA1B2_C3D4));
        crate::test_guest_memory::unmap_guest_page(page);
    }
}
