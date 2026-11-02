// SPDX-License-Identifier: GPL-3.0-or-later
//! Tier 0 behavioural smoke: these are spec checks, not lifted differential evidence.
mod support;
use support::Mem;
use tsfp_engine_core::{initializers::*, launch::*};
use tsfp_engine_object::closure::{
    callee::{Callee, Cpu},
    memory::GuestMemory,
};
impl Mem {
    fn string(&mut self, a: u32, s: &[u8]) {
        for (i, v) in s.iter().enumerate() {
            self.write8(a + i as u32, *v);
        }
    }
}
#[derive(Default)]
struct Calls {
    calls: Vec<(u32, u32, Vec<u32>)>,
    result: u32,
}
impl Callee<Mem> for Calls {
    fn call(&mut self, m: &mut Mem, t: u32, cpu: &mut Cpu) {
        let n = match t {
            0x3800BF | 0x37FC76 | 0x37C9F6 | 0x1B5300 => 2,
            _ => 1,
        };
        self.calls.push((
            t,
            m.read32(cpu.esp),
            (0..n).map(|i| m.read32(cpu.esp + 4 + i * 4)).collect(),
        ));
        if t == 0x37C9F6 {
            assert_eq!(m.read32(m.read32(cpu.esp + 8) + 4), 0);
        }
        cpu.eax = self.result;
    }
}
fn setup(args: &[u32]) -> (Mem, Cpu, Calls) {
    let mut m = Mem::default();
    for (i, v) in args.iter().enumerate() {
        m.write32(0x9004 + i as u32 * 4, *v);
    }
    (
        m,
        Cpu {
            esp: 0x9000,
            ..Cpu::default()
        },
        Calls {
            result: 0xFFFF_FFFE,
            ..Calls::default()
        },
    )
}
#[test]
fn lookup_first_match_case_sensitive_and_misses() {
    let (mut m, mut cpu, _) = setup(&[0x4000]);
    m.string(0x4000, b"mode\0");
    m.string(0x4100, b"Mode\0");
    m.string(0x4200, b"mode\0");
    m.write32(0x74CF68, 4);
    m.write32(0x74CF6C, 0x5000);
    m.write32(0x5004, 0x4100);
    m.write32(0x5008, 0x4200);
    m.write32(0x500C, 0x4200);
    find_argument(&mut m, &mut cpu);
    assert_eq!((cpu.eax, m.read32(0x6B7A20)), (2, 2));
    m.string(0x4000, b"nope\0");
    find_argument(&mut m, &mut cpu);
    assert_eq!((cpu.eax, m.read32(0x6B7A20)), (0, 0));
    m.write8(0x4000, 0);
    m.write32(0x74CF68, u32::MAX);
    find_argument(&mut m, &mut cpu);
    assert_eq!(cpu.eax, 0);
}
#[test]
fn numeric_handlers_preserve_width_and_call_sites() {
    let cases = [
        (0x3C920, 0x7EB354, 0x3C92A, true),
        (0x3C970, 0x7EB300, 0x3C97A, true),
        (0x3C990, 0x7EB302, 0x3C99A, true),
        (0x3D1A0, 0x7EB374, 0x3D1AA, false),
        (0x3D1C0, 0x7EB368, 0x3D1CA, false),
        (0x3D1E0, 0x7EB364, 0x3D1EA, false),
    ];
    for (va, address, pc, byte) in cases {
        let (mut m, mut cpu, mut c) = setup(&[0x4000]);
        m.write8(address + 4, 0xAC);
        assert!(run_if_enabled(va, true, &mut c, &mut m, &mut cpu));
        assert_eq!(c.calls, [(0x3C8FD0, pc, vec![0x4000])]);
        assert_eq!(cpu.eax, 1);
        assert_eq!(cpu.esp, 0x9000);
        if byte {
            assert_eq!(m.read8(address), 0xFE);
        } else {
            assert_eq!(m.read32(address), u32::MAX - 1);
        }
        assert_eq!(m.read8(address + 4), 0xAC);
        if va == 0x3C970 {
            assert_eq!(m.read8(address + 1), 0xFE);
        }
    }
}
#[test]
fn flag_order_signed_exit_and_wrapping_scale() {
    let (mut m, mut cpu, mut c) = setup(&[0x4000]);
    m.write32(0x7EB358, 0x123);
    set_named_player(&mut c, &mut m, &mut cpu);
    assert_eq!(m.read32(0x7EB358), 0x20000123);
    assert_eq!(m.read8(0x7EB39B), 0xFE);
    assert_eq!(c.calls[0].0, 0x6F450);
    c.calls.clear();
    set_global_count(&mut c, &mut m, &mut cpu);
    assert_eq!(m.read32(0x6B7A88), u32::MAX - 1);
    assert_eq!(c.calls[1], (0x389B0, 0x3D15E, vec![u32::MAX]));
    m.write32(0x4E8CEC, 3);
    set_scaled_time(&mut c, &mut m, &mut cpu);
    assert_eq!(m.read32(0x7EB360), 0xFFFF_FFFA);
    set_special_mode(&mut c, &mut m, &mut cpu);
    assert_eq!(m.read8(0x7EB39F), 0xFE);
    assert_eq!(m.read8(0x7EB357), 1);
    assert_eq!(m.read32(0x7EB358), 0x20800123);
}
#[test]
fn fixed_byte_matches_and_unchanged_unknown_variant() {
    let (mut m, mut cpu, _) = setup(&[0x4000]);
    for (constant, n, value) in [(0x4780A8, 5, 0), (0x4780A0, 7, 1), (0x478098, 5, 2)] {
        m.string(0x4000, b"xyz1234");
        m.string(constant, b"xyz1234");
        set_variant_name(&mut m, &mut cpu);
        assert_eq!(m.read8(0x7EB357), value);
        m.write8(constant, 0);
        assert!(n > 0);
    }
    m.write8(0x7EB357, 99);
    set_variant_name(&mut m, &mut cpu);
    assert_eq!(m.read8(0x7EB357), 99);
    m.string(0x4780B0, b"xyz1234");
    set_global_switch(&mut m, &mut cpu);
    assert_eq!(m.read32(0x6B7A8C), 1);
    m.write8(0x4780B6, 0);
    set_global_switch(&mut m, &mut cpu);
    assert_eq!(m.read32(0x6B7A8C), 0);
}
#[test]
fn launch_payload_tail_and_thunks() {
    for (va, kind, pc) in [
        (0x31AB0, 8, 0x31AD6),
        (0x31AE0, 6, 0x31B06),
        (0x31B10, 7, 0x31B36),
    ] {
        let (mut m, mut cpu, mut c) = setup(&[]);
        m.write32(0x8410, 0xABCD);
        run_if_enabled(va, true, &mut c, &mut m, &mut cpu);
        assert_eq!(m.read32(0x8400), kind);
        assert_eq!(m.read32(0x8410), 0xABCD);
        assert_eq!(c.calls[0], (0x37C9F6, pc, vec![0, 0x8400]));
        assert_eq!(cpu.esp, 0x9000);
    }
    for (va, target) in [(0x372A0, 0x31AB0), (0x372B0, 0x31AE0), (0x37300, 0x31B10)] {
        let (mut m, mut cpu, mut c) = setup(&[17]);
        m.write32(0x9000, 0xCAFEBABE);
        run_if_enabled(va, true, &mut c, &mut m, &mut cpu);
        assert_eq!(c.calls[0], (target, 0xCAFEBABE, vec![17]));
    }
}
#[test]
fn launch_worker_active_and_inactive() {
    let (mut m, mut cpu, mut c) = setup(&[]);
    join_launch_worker(&mut c, &mut m, &mut cpu);
    assert!(c.calls.is_empty());
    m.write32(0x687E24, 1);
    m.write32(0x687DDC, 44);
    m.write32(0x687DE0, 55);
    join_launch_worker(&mut c, &mut m, &mut cpu);
    assert_eq!(
        c.calls,
        [
            (0x3800BF, 0x2FF80, vec![44, u32::MAX]),
            (0x37CBC4, 0x2FF8C, vec![44]),
            (0x37FC76, 0x2FF9A, vec![55, 0])
        ]
    );
    assert_eq!(m.read32(0x687E20), 1);
}
#[test]
fn constructors_cover_final_words_and_threshold_interleave() {
    let (mut m, cpu, _) = setup(&[0x4000, 0xDEAD]);
    m.write32(0x4084, 0xABCD);
    init_slot_arrays(&mut m, &cpu);
    for i in 0..33 {
        assert_eq!(m.read32(0x4000 + 4 * i), if i < 11 { 0xDEAD } else { 0 });
    }
    assert_eq!(m.read32(0x4084), 0xABCD);
    assert_eq!(ThresholdMasks::new([4, 2, 1]).masks, [5, 2, 0]);
    assert_eq!(ThresholdMasks::new([0, 0, 0]).masks, [0; 3]);
    let (mut m, mut cpu, _) = setup(&[4, 2, 1]);
    cpu.ecx = 0x5000;
    init_threshold_masks(&mut m, &cpu);
    assert_eq!(m.read32(0x500C), 5);
    assert_eq!(m.read32(0x5020), 0);
    for flags in 0..1024 {
        let (mut m, mut cpu, _) = setup(&[0x4000, flags]);
        init_flag_addresses(&mut m, &mut cpu);
        for (i, bit) in [1, 2, 4, 8, 16, 32, 64, 512, 256].iter().enumerate() {
            assert_eq!(
                m.read32(0x4004 + i as u32 * 8),
                if flags & bit != 0 { 0x4768E8 } else { 0x4768EC }
            );
        }
        assert_eq!(m.read32(0x404C), 0x4768F8);
    }
}
#[test]
fn opt_in_off_and_unsupported_do_not_mutate() {
    let (mut m, mut cpu, mut c) = setup(&[0x4000]);
    let before = cpu;
    assert!(!run_if_enabled(0x3C920, false, &mut c, &mut m, &mut cpu));
    assert!(!run_if_enabled(0xABCDEF, true, &mut c, &mut m, &mut cpu));
    assert_eq!(cpu, before);
    assert!(c.calls.is_empty());
}

#[test]
fn launch_entry_preserves_nonreturning_state_contract() {
    let (mut m, mut cpu, mut c) = setup(&[]);
    m.write32(0x74CF68, 3);
    m.write32(0x74CF6C, 0x4000);
    m.write32(0x7EB354, 0xAAAA);
    let exit = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        parse_launch_and_enter_state(&mut c, &mut m, &mut cpu)
    }));
    assert!(exit.is_err());
    assert_eq!(m.read32(0x7EB354), 0);
    assert_eq!(
        c.calls.iter().map(|x| x.0).collect::<Vec<_>>(),
        [0x1B5300, 0x658D0, 0x3DD20, 0x389B0, 0x3D6A0]
    );
    assert_eq!(c.calls[0], (0x1B5300, 0x3C2F1, vec![3, 0x4000]));
    assert_eq!(c.calls[4], (0x3D6A0, 0x3C323, vec![1]));
}
/// T2409: the original scans in registers and writes the cursor only on exit, so a name that lives
/// on the cursor word (0x6B7A20) is compared unmodified. Regression for the cursor-first-write bug.
#[test]
fn lookup_name_on_cursor_word_is_not_clobbered() {
    let (mut m, mut cpu, _) = setup(&[0x6B7A20]);
    m.write32(0x6B7A20, 0x0000_6261); // "ab\0"
    m.string(0x4100, b"ab\0");
    m.write32(0x74CF68, 2);
    m.write32(0x74CF6C, 0x5000);
    m.write32(0x5004, 0x4100);
    find_argument(&mut m, &mut cpu);
    assert_eq!((cpu.eax, m.read32(0x6B7A20)), (1, 1));
}
