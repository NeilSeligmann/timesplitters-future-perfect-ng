//! Differential check of the ui port vs the original-verified tsfp-ports body.
mod memory {
    pub trait GuestMemory {
        fn read8(&self, address: u32) -> u8;
        fn write8(&mut self, address: u32, value: u8);
        fn read16(&self, address: u32) -> u16 {
            u16::from_le_bytes([self.read8(address), self.read8(address.wrapping_add(1))])
        }
        fn read32(&self, address: u32) -> u32 {
            u32::from_le_bytes(core::array::from_fn(|i| {
                self.read8(address.wrapping_add(i as u32))
            }))
        }
        fn write32(&mut self, address: u32, value: u32) {
            for (i, byte) in value.to_le_bytes().into_iter().enumerate() {
                self.write8(address.wrapping_add(i as u32), byte);
            }
        }
    }
}
#[path = "../../tsfp-ports/src/behaviour.rs"]
#[allow(dead_code)]
mod behaviour_src;
use memory::GuestMemory;
use tsfp_ui::UiMemory;

const SIZE: usize = 0x400;
struct Mem(Vec<u8>);
fn idx(a: u32) -> usize {
    a as usize % SIZE
}
impl UiMemory for Mem {
    fn read8(&self, a: u32) -> u8 {
        self.0[idx(a)]
    }
}
impl GuestMemory for Mem {
    fn read8(&self, a: u32) -> u8 {
        self.0[idx(a)]
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0[idx(a)] = v
    }
}
#[path = "support/rng.rs"]
mod rng;
use rng::Rng;

#[test]
fn has_bit_agrees() {
    let mut r = Rng(0x2026_1010_1836);
    for _ in 0..50000 {
        let m = Mem((0..SIZE).map(|_| r.next() as u8).collect());
        let object = match r.next() % 3 {
            0 => r.next() % SIZE as u32,
            1 => 0u32.wrapping_sub(r.next() % 0xb0),
            _ => r.next(),
        };
        let bit = if r.next() % 2 == 0 {
            r.next() % 40
        } else {
            r.next()
        };
        assert_eq!(
            tsfp_ui::record_a0_has_bit(&m, object, bit),
            behaviour_src::has_bit(&m, object, bit)
        );
    }
}

#[test]
fn shift_count_is_masked_and_wraps() {
    let mut m = Mem(vec![0; SIZE]);
    m.0[0xa0] = 1;
    assert_eq!(tsfp_ui::record_a0_has_bit(&m, 0, 32), 1);
    assert_eq!(tsfp_ui::record_a0_has_bit(&m, 0, 1), 0);
    let mut w = Mem(vec![0; SIZE]);
    w.0[0] = 1;
    assert_eq!(
        tsfp_ui::record_a0_has_bit(&w, 0u32.wrapping_sub(0xa0), 0),
        1
    );
}

#[test]
fn mutant_is_detected() {
    let mut r = Rng(9);
    let m = Mem((0..SIZE).map(|_| r.next() as u8).collect());
    let diffs = (0..64u32)
        .filter(|b| {
            let mutant = u32::from(UiMemory::read32(&m, 0xa0) & (1u32 << (b % 32)) == 0);
            mutant != behaviour_src::has_bit(&m, 0, *b)
        })
        .count();
    assert!(diffs > 0);
}
