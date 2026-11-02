//! Guest memory contracts for the native file boundary (T3879).
//! Addresses are checked by the existing host memory service; no guest address
//! is dereferenced as a Rust pointer. The ANSI and pseudo-root policies remain
//! the documented host model, not a claim about physical hardware.
use tsfp_engine_fileio::guest_names::{self, Address, NameMemory, RootPath};

pub const INVALID_PARAMETER: u32 = 0xc000000d;
pub const PATH_NOT_FOUND: u32 = 0xc000003a;
pub const BUFFER_TOO_SMALL: u32 = 0xc0000023;
pub const PATH_CAPACITY: usize = 261;

pub trait Memory: NameMemory {
    fn put_byte(&mut self, address: Address, value: u8) -> bool;
    fn put_word(&mut self, address: Address, value: u32) -> bool;
}

/// The caller's OBJECT_ATTRIBUTES is three words, with no desktop NT Length.
pub fn attributes(memory: &mut impl NameMemory, address: Address) -> Option<[u32; 3]> {
    let mut values = [0; 3];
    guest_names::read_attributes(memory, address, |_| true, |field, value| {
        use guest_names::AttributeField::*;
        values[match field { RootDirectory => 0, ObjectName => 1, Attributes => 2 }] = value;
    }).then_some(values)
}

pub fn name(memory: &mut impl NameMemory, address: Address) -> Result<Vec<u8>, u32> {
    if address.0 == 0 { return Err(INVALID_PARAMETER); }
    let mut bytes = [0; PATH_CAPACITY];
    if !guest_names::read_name(memory, address, &mut bytes) {
        return Err(INVALID_PARAMETER);
    }
    let length = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    Ok(bytes[..length].to_vec())
}

pub fn rooted(root: u32, path: &[u8], capacity: usize) -> Result<Vec<u8>, u32> {
    match guest_names::root_path(root, path, capacity) {
        RootPath::Unchanged => Ok(path.to_vec()),
        RootPath::DriveAbsolute => {
            let mut expanded = b"\\??\\".to_vec();
            expanded.extend_from_slice(path);
            Ok(expanded)
        }
        RootPath::Refused => Err(PATH_NOT_FOUND),
    }
}

/// Preserve partial writes and the original optional null IO_STATUS_BLOCK.
pub fn io_status(memory: &mut impl Memory, address: Address, status: u32, information: u32) -> bool {
    address.0 == 0 || (memory.put_word(address, status)
        && memory.put_word(address.field(4), information))
}

/// A symbolic-link query writes bytes without a trailing NUL. On insufficient
/// capacity it reports the required length and leaves the output string intact.
pub fn write_link(memory: &mut impl Memory, string: Address, returned: Address, target: &[u8]) -> u32 {
    let Some(low) = memory.byte(string.field(2)) else { return INVALID_PARAMETER };
    let Some(high) = memory.byte(string.field(3)) else { return INVALID_PARAMETER };
    let Some(buffer) = memory.word(string.field(4)) else { return INVALID_PARAMETER };
    if string.0 == 0 || buffer == 0 { return INVALID_PARAMETER; }
    let capacity = u16::from_le_bytes([low, high]) as usize;
    if target.len() > capacity {
        if returned.0 != 0 { let _ = memory.put_word(returned, target.len() as u32); }
        return BUFFER_TOO_SMALL;
    }
    for (index, &byte) in target.iter().enumerate() {
        if !memory.put_byte(Address(buffer).field(index as u32), byte) {
            return INVALID_PARAMETER;
        }
    }
    let length = (target.len() as u16).to_le_bytes();
    if !memory.put_byte(string, length[0])
        || !memory.put_byte(string.field(1), length[1])
        || (returned.0 != 0 && !memory.put_word(returned, target.len() as u32)) {
        return INVALID_PARAMETER;
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake { bytes: [u8; 128], writes: Vec<(u32, u8)>, fail: Option<u32> }
    impl Fake {
        fn new() -> Self { Self { bytes: [0; 128], writes: vec![], fail: None } }
        fn word_at(&mut self, address: usize, value: u32) {
            self.bytes[address..address + 4].copy_from_slice(&value.to_le_bytes());
        }
    }
    impl NameMemory for Fake {
        fn byte(&mut self, a: Address) -> Option<u8> {
            (a.0 != 0).then(|| self.bytes.get(a.0 as usize).copied()).flatten()
        }
        fn word(&mut self, a: Address) -> Option<u32> {
            if a.0 == 0 { return None; }
            self.bytes.get(a.0 as usize..a.0 as usize + 4)
                .map(|v| u32::from_le_bytes(v.try_into().unwrap()))
        }
    }
    impl Memory for Fake {
        fn put_byte(&mut self, a: Address, v: u8) -> bool {
            if a.0 == 0 || self.fail == Some(a.0) { return false; }
            let Some(slot) = self.bytes.get_mut(a.0 as usize) else { return false };
            *slot = v;
            self.writes.push((a.0, v));
            true
        }
        fn put_word(&mut self, a: Address, v: u32) -> bool {
            v.to_le_bytes().iter().enumerate().all(|(i, &b)| self.put_byte(a.field(i as u32), b))
        }
    }
    #[test]
    fn short_link_query_publishes_only_required_length() {
        let mut mem = Fake::new();
        mem.bytes[10] = 2;
        mem.word_at(12, 64);
        assert_eq!(write_link(&mut mem, Address(8), Address(24), b"disc"), BUFFER_TOO_SMALL);
        assert_eq!(mem.word(Address(24)), Some(4));
        assert_eq!(&mem.bytes[64..68], &[0; 4]);
        assert!(mem.writes.iter().all(|(a, _)| (24..28).contains(a)));
    }
    #[test]
    fn exact_capacity_link_query_has_no_nul_or_unrelated_write() {
        let mut mem = Fake::new();
        mem.bytes[10] = 4;
        mem.bytes[68] = 0xee;
        mem.word_at(12, 64);
        assert_eq!(write_link(&mut mem, Address(8), Address(24), b"disc"), 0);
        assert_eq!(&mem.bytes[64..69], b"disc\xee");
        assert_eq!(&mem.bytes[8..10], &[4, 0]);
        assert_eq!(mem.word(Address(24)), Some(4));
    }
    #[test]
    fn link_copy_failure_does_not_publish_length() {
        let mut mem = Fake::new();
        mem.bytes[10] = 8;
        mem.word_at(12, 64);
        mem.fail = Some(66);
        assert_eq!(write_link(&mut mem, Address(8), Address(24), b"disc"), INVALID_PARAMETER);
        assert_eq!(&mem.bytes[64..68], b"di\0\0");
        assert_eq!(&mem.bytes[8..10], &[0, 0]);
        assert_eq!(mem.word(Address(24)), Some(0));
    }
    #[test]
    fn optional_status_and_failed_status_stop_publication() {
        let mut mem = Fake::new();
        assert!(io_status(&mut mem, Address(0), 0, 1));
        assert!(mem.writes.is_empty());
        mem.fail = Some(8);
        assert!(!io_status(&mut mem, Address(8), 0xc000000d, 1));
        assert!(mem.writes.is_empty());
    }
    #[test]
    fn attributes_reject_wrapping_guest_address() {
        let mut mem = Fake::new();
        assert!(attributes(&mut mem, Address(u32::MAX - 3)).is_none());
    }
}
