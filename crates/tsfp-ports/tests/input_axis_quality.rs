use std::collections::BTreeMap;
use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    memory::GuestMemory,
};
#[unsafe(no_mangle)]
extern "C" fn xbox_ReadTimeStampCounter() -> u64 {
    0
}
#[derive(Clone, Debug, PartialEq, Eq, Default)]
struct Ram(BTreeMap<u32, u8>);
impl GuestMemory for Ram {
    fn read8(&self, a: u32) -> u8 {
        *self.0.get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0.insert(a, v);
    }
}
fn regs() -> ExtRegisters {
    ExtRegisters {
        eax: 7,
        ecx: 8,
        edx: 9,
        esp: 0x9000,
        ebx: 10,
        ebp: 11,
        esi: 12,
        edi: 13,
        eflags: 0x202,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 0,
            control: 0x27f,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}
#[test]
fn axis_reader_matches_translated_on_fabricated_pad_records() {
    let mut seed = 0x42970u32;
    for case in 0..3000 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let mut expected = Ram::default();
        expected.write32(0x6b9888, u32::from(case % 11 == 0));
        expected.write32(0x76b0fc, u32::from(case % 2 == 0));
        expected.write32(0x9004, 1);
        expected.write32(0x9008, case % 4);
        expected.write32(0x6b93c4, 2);
        let (base, index) = if case % 2 == 0 {
            (0x6b94c8, 1)
        } else {
            (0x6b93d0, 2)
        };
        let record = base + index * 0x3c;
        expected.write32(
            record,
            match case % 5 {
                0 => 0.0f32,
                1 => 0.1,
                2 => 0.5,
                3 => -0.1,
                _ => 1.0,
            }
            .to_bits(),
        );
        expected.write32(record + 0x18 + (case % 4) * 4, seed % 256);
        expected.write32(0x476038, (1.0f32 / 127.0).to_bits());
        expected.write32(0x4783a8, (1.0f32 / 128.0).to_bits());
        expected.write32(0x475c78, 1.0f32.to_bits());
        expected.write32(0x475cac, 0.25f32.to_bits());
        let mut actual = expected.clone();
        let mut a = regs();
        let mut b = a;
        tsfp_ports::c2r::engine_input::game_input_pad_axis_get_float_with_deadzone(
            &mut expected,
            &mut a,
        );
        assert!(tsfp_ports::input_lane::run(
            0x42970,
            &mut actual,
            &mut NoCalls,
            &mut b
        ));
        assert_eq!(actual, expected, "case {case}");
        assert_eq!(b, a, "case {case}");
    }
}

struct NoCalls;
impl GuestCall for NoCalls {
    fn call(&mut self, _: u32, _: &mut ExtRegisters) {
        panic!("unexpected guest call")
    }
}
