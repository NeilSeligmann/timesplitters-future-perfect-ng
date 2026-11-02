use tsfp_kernel_services::{GuestMemory, debug};

struct Memory(Vec<u8>);

impl GuestMemory for Memory {
    fn readable(&self, address: u32, length: usize) -> bool {
        self.0
            .get(address as usize..address as usize + length)
            .is_some()
    }

    fn read(&self, address: u32, output: &mut [u8]) -> bool {
        let Some(bytes) = self
            .0
            .get(address as usize..address as usize + output.len())
        else {
            return false;
        };
        output.copy_from_slice(bytes);
        true
    }

    fn write(&mut self, _: u32, _: &[u8]) -> bool {
        false
    }
}

#[test]
fn conversions_and_unknown_specifier_keep_argument_order() {
    let memory = Memory(b"_%q|%d|%u|%x|%X|%p|%c|%%|%s\0text\0".to_vec());
    let string_address = memory.0.iter().position(|&byte| byte == 0).unwrap() + 1;
    let args = [
        1,
        u32::MAX,
        17,
        0xab,
        0xcd,
        0x123,
        b'Z' as u32,
        string_address as u32,
    ];
    assert_eq!(
        debug::format(&memory, 1, &args, 128),
        b"%q|-1|17|ab|CD|0x00000123|Z|%|text"
    );
}

#[test]
fn capacity_and_guest_fault_grid() {
    let memory = Memory(b"_abc%u:%s\0".to_vec());
    for capacity in 0..=16 {
        let full = b"abc42:<bad %s ptr>";
        let result = debug::format(&memory, 1, &[1, 42, 999], capacity);
        assert_eq!(result, full[..full.len().min(capacity.saturating_sub(1))]);
    }
    assert_eq!(
        debug::format(&Memory(b"_%d\0".to_vec()), 1, &[1], 32),
        b"<arg?>"
    );
    assert_eq!(
        debug::format(&Memory(b"_%s\0x".to_vec()), 1, &[1, 4], 32),
        b"x<unterminated>"
    );
}
