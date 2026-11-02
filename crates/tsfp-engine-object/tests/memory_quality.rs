use std::collections::HashMap;
use tsfp_engine_object::{ObjectMemory, request_state};

#[derive(Default)]
struct Memory(HashMap<u32, u8>);
impl ObjectMemory for Memory {
    fn read8(&self, address: u32) -> u8 {
        *self.0.get(&address).unwrap_or(&0)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.insert(address, value);
    }
}

#[test]
fn multibyte_accesses_wrap_and_preserve_neighbors() {
    for offset in 0..8 {
        let base = u32::MAX - offset;
        let mut memory = Memory::default();
        for address in [base.wrapping_sub(1), base.wrapping_add(4)] {
            memory.write8(address, 0x5a);
        }
        for seed in 0..256_u32 {
            let value = seed.wrapping_mul(0x0102_0407) ^ 0xa5c3_6e91;
            memory.write32(base, value);
            assert_eq!(memory.read32(base), value);
            assert_eq!(memory.read16(base), value as u16);
            assert_eq!(memory.read8(base.wrapping_sub(1)), 0x5a);
            assert_eq!(memory.read8(base.wrapping_add(4)), 0x5a);
        }
    }
}

#[test]
fn state_request_reads_alias_after_previous_state_store() {
    for current in [0, 1, 7, 9, 0xffff_ffff] {
        let object = u32::MAX - 0x80;
        let mut memory = Memory::default();
        memory.write32(object.wrapping_add(0x114), current);
        memory.write32(object.wrapping_add(0x110), 0x8765_4321);
        memory.write32(object.wrapping_add(0x118), 99);
        memory.write32(object.wrapping_add(0x0c), 0xffff_ffff);
        assert_eq!(
            request_state(&mut memory, object, object.wrapping_add(0x110)),
            0xfffd_fbff
        );
        assert_eq!(memory.read32(object.wrapping_add(0x110)), current);
        assert_eq!(memory.read32(object.wrapping_add(0x114)), current);
        assert_eq!(memory.read32(object.wrapping_add(0x118)), 0);
    }
}
