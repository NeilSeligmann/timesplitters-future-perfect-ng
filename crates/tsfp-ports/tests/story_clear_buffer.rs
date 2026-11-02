use std::collections::BTreeMap;
use tsfp_ports::abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use tsfp_ports::memory::GuestMemory;

#[unsafe(no_mangle)]
extern "C" fn xbox_ReadTimeStampCounter() -> u64 { 0 }

#[derive(Default, Clone, PartialEq, Eq, Debug)]
struct Memory { bytes: BTreeMap<u32, u8>, stores: Vec<(u32, u32)> }
impl GuestMemory for Memory {
    fn read8(&self, address: u32) -> u8 { *self.bytes.get(&address).unwrap_or(&0) }
    fn write8(&mut self, address: u32, value: u8) {
        self.bytes.insert(address, value);
        self.stores.push((address, 1));
    }
    fn write32(&mut self, address: u32, value: u32) {
        for (offset, byte) in value.to_le_bytes().into_iter().enumerate() {
            self.bytes.insert(address.wrapping_add(offset as u32), byte);
        }
        self.stores.push((address, 4));
    }
}

#[test]
fn readable_story_clear_matches_generated_for_fabricated_buffers() {
    let mut seed = 0x227c00u32;
    for _ in 0..2_000 {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let base = if seed & 7 == 0 { 0 } else if seed & 7 == 1 { u32::MAX - 63 } else { 0x2000 };
        let size = (seed >> 8) & 127;
        let mut before = Memory::default();
        before.write32(0x75c148, base);
        before.write32(0x75c150, size);
        before.stores.clear();
        let mut expected_memory = before.clone();
        let mut actual_memory = before;
        let mut expected = regs();
        let mut actual = expected;
        tsfp_ports::c2r::t2049_remote2_modes_001::game_zero_fill_buffer_75c148_of_size(
            &mut expected_memory, &mut expected,
        );
        assert!(tsfp_ports::c2r::run(0x227c00, &mut actual_memory, &mut NoCalls, &mut actual));
        assert_eq!(actual_memory, expected_memory, "seed {seed:#x}");
        assert_eq!(actual, expected, "seed {seed:#x}");
    }
}
struct NoCalls;
impl GuestCall for NoCalls { fn call(&mut self, _: u32, _: &mut ExtRegisters) { panic!("unexpected guest call") } }
fn regs() -> ExtRegisters {
    ExtRegisters { eax: 7, ecx: 8, edx: 9, esp: 0x9000, ebx: 10, ebp: 11, esi: 12, edi: 13,
        eflags: 0x202, fpu: Fpu { stack: [0.0; 8], top: 0, control: 0x27f },
        xmm: [Xmm::default(); 5], xmm_hi: [Xmm::default(); 3] }
}
#[test]
fn story_buffer_clear_preserves_store_widths_and_registers() {
    for (base, size) in [(0, 5), (0x2000, 0), (0x2000, 1), (0x2000, 3), (0x2000, 4), (0x2000, 7)] {
        let mut memory = Memory::default();
        memory.write32(0x75c148, base);
        memory.write32(0x75c150, size);
        for offset in 0..9 { memory.write8(0x2000 + offset, 0xaa); }
        memory.stores.clear();
        let mut r = regs();
        assert!(tsfp_ports::c2r::run(0x227c00, &mut memory, &mut NoCalls, &mut r));
        let mut expected_stores = vec![(0x8ffc, 4)]; // saved EDI
        if base != 0 && size != 0 {
            expected_stores.extend(
            (0..size / 4).map(|i| (base + 4 * i, 4))
                .chain((0..size % 4).map(|i| (base + size / 4 * 4 + i, 1))));
        }
        assert_eq!(memory.stores, expected_stores);
        for offset in 0..9 { assert_eq!(memory.read8(0x2000 + offset), if base != 0 && offset < size { 0 } else { 0xaa }); }
        let mut expected = regs();
        if base != 0 { expected.ecx = size; }
        if base != 0 && size != 0 {
            expected.eax = 0;
            expected.ecx = 0;
            expected.edx = size;
        }
        assert_eq!(r, expected);
    }
}
