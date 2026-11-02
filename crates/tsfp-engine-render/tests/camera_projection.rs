use std::collections::BTreeMap;
use tsfp_engine_render::camera_projection::*;
use tsfp_ports::abi::{Fpu, Xmm};
use tsfp_ports::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
};

#[derive(Clone, Default, Debug, PartialEq)]
struct Memory(BTreeMap<u32, u8>);
impl GuestMemory for Memory {
    fn read8(&self, address: u32) -> u8 {
        *self.0.get(&address).unwrap_or(&0)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.insert(address, value);
    }
}
#[derive(Default, Debug, PartialEq)]
struct Calls(Vec<u32>);
impl GuestCall for Calls {
    fn call(&mut self, va: u32, _: &mut ExtRegisters) {
        self.0.push(va);
    }
}
fn registers() -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp: 0x1000,
        ebx: 11,
        ebp: 12,
        esi: 13,
        edi: 14,
        eflags: 0x202,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 0,
            control: 0x37f,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}
#[test]
fn fabricated_projection_matches_translated_root() {
    let mut seed = 0x2285_u32;
    for case in 0..2048 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let mut a = Memory::default();
        a.write32(0x7e71c8, 0x4000);
        a.write32(0x400c, 320 + (seed % 1600));
        a.write32(0x4020, 240 + ((seed >> 8) % 800));
        a.write32(0x7b0cc0, 0x3000);
        a.write32(0x3204, 0x5000);
        a.write32(0x1004, 0x5100);
        a.write32(0x1008, 0x5200);
        a.write32(0x100c, (0.5 + (seed % 100) as f32 / 100.0).to_bits());
        a.write32(0x1010, (20.0 + (seed % 100) as f32).to_bits());
        a.write32(0x7de458, if case % 2 == 0 { 0x4000_0000 } else { 0 });
        a.write32(0x4760cc, (1.0 + (seed % 300) as f32 / 100.0).to_bits());
        a.write32(0x475d0c, (core::f32::consts::PI / 180.0).to_bits());
        a.write32(0x475cd4, 0.5_f32.to_bits());
        let mut b = a.clone();
        let mut ar = registers();
        let mut br = ar;
        let mut ac = Calls::default();
        let mut bc = Calls::default();
        tsfp_engine_render::scene_batch::game_camera_set_perspective_from_fov_and_viewport_aspect(
            &mut a, &mut ac, &mut ar,
        );
        run(&mut b, &mut bc, &mut br);
        assert_eq!(a, b, "memory seed {case}");
        assert_eq!(ar, br, "register seed {case}");
        assert_eq!(ac, bc, "call seed {case}");
    }
}

#[test]
fn hor_plus_runtime_preserves_vertical_and_expands_horizontal() {
    let mut original = Memory::default();
    original.write32(0x7e71c8, 0x4000);
    original.write32(0x400c, 640);
    original.write32(0x4020, 480);
    original.write32(0x7b0cc0, 0x3000);
    original.write32(0x3204, 0x5000);
    original.write32(0x100c, 1.0_f32.to_bits());
    original.write32(0x1010, 55.0_f32.to_bits());
    original.write32(0x475d0c, (core::f32::consts::PI / 180.0).to_bits());
    original.write32(0x475cd4, 0.5_f32.to_bits());
    let mut baseline = original.clone();
    run_with_preset(
        &mut baseline,
        &mut Calls::default(),
        &mut registers(),
        ProjectionPreset::Original,
    );
    let vertical = f32::from_bits(baseline.read32(0x337c));
    let horizontal = f32::from_bits(baseline.read32(0x3380));
    for aspect in [4.0 / 3.0, 16.0 / 9.0, 7.0 / 3.0] {
        let mut memory = original.clone();
        run_with_preset(
            &mut memory,
            &mut Calls::default(),
            &mut registers(),
            ProjectionPreset::hor_plus(aspect).unwrap(),
        );
        assert_eq!(memory.read32(0x337c), vertical.to_bits());
        let actual = f32::from_bits(memory.read32(0x3380));
        let expected = vertical * aspect;
        assert!(
            (actual - expected).abs() < 0.000001,
            "{aspect}: {actual} != {expected}"
        );
        if aspect == 4.0 / 3.0 {
            assert_eq!(memory.read32(0x3380), horizontal.to_bits());
        }
    }
    assert!(ProjectionPreset::hor_plus(f32::NAN).is_none());
    assert!(ProjectionPreset::hor_plus(0.0).is_none());
}
