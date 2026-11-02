// SPDX-License-Identifier: GPL-3.0-or-later
// T1831. FABRICATED synthetic fixtures with a scripted callee closure. Behavioural checks
// derived from the lifted originals; the original-vs-Rust equivalence is tests/c/test_rust_weapons.c.
use abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use memory::GuestMemory;
use std::collections::BTreeMap;
use tsfp_port_runtime::{abi, memory};
use tsfp_ports::{combat, ext, weapons};
#[derive(Default)]
struct Mem(BTreeMap<u32, u8>);
impl GuestMemory for Mem {
    fn read8(&self, a: u32) -> u8 {
        *self.0.get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0.insert(a, v);
    }
}
type Seen = (u32, [u32; 6], Vec<u32>);
#[derive(Default)]
struct Guest {
    seen: Vec<Seen>,
    results: Vec<u32>,
}
impl GuestCall for Guest {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        let m_args = vec![r.esp];
        self.seen
            .push((va, [r.eax, r.ecx, r.edx, r.esi, r.edi, r.esp], m_args));
        r.eax = if self.results.is_empty() {
            0
        } else {
            self.results.remove(0)
        };
        r.ecx = 0xc0c0;
        r.edx = 0xd0d0;
    }
}
const STACK: u32 = 0x9000;
fn regs() -> ExtRegisters {
    ExtRegisters {
        eax: 1,
        ecx: 2,
        edx: 3,
        esp: STACK,
        ebx: 0xb,
        ebp: 0xe,
        esi: 0x51,
        edi: 0xd1,
        eflags: 0,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 0,
            control: 0x27f,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}
fn with_args(m: &mut Mem, args: &[u32]) {
    m.write32(STACK, 0x1234);
    for (i, a) in args.iter().enumerate() {
        m.write32(STACK + 4 + 4 * i as u32, *a);
    }
}
#[test]
fn b5660_selects_entry_or_base_fallback() {
    let mut m = Mem::default();
    for (first, expect_entry) in [
        (0u32, true),
        (5, true),
        (0x8000_0000, false),
        (0xffff_ffff, false),
    ] {
        m.write32(0x2000 + 3 * 0x7c + 0x68, first);
        with_args(&mut m, &[0x2000, 3]);
        let mut r = regs();
        weapons::b5660(&mut m, &mut r);
        let entry = 0x2000 + 3 * 0x7c + 0x68;
        assert_eq!(r.eax, if expect_entry { entry } else { 0x2068 });
        assert_eq!((r.ecx, r.edx, r.esp), (0x2000, first, STACK));
    }
}
#[test]
fn b5660_negative_index_wraps_like_imul() {
    let mut m = Mem::default();
    with_args(&mut m, &[0x2000, 0xffff_ffff]);
    let mut r = regs();
    weapons::b5660(&mut m, &mut r);
    assert_eq!(r.eax, 0x2000u32.wrapping_sub(0x7c).wrapping_add(0x68));
}
fn ammo_fixture(have: i16, held: i16, cap: u32, flag: u32) -> (Mem, ExtRegisters) {
    let mut m = Mem::default();
    m.write32(weapons::DEFINITION_TABLE, 0x5000);
    m.write32(0x3000 + 0x30, 2); // state definition index
    let definition = 0x5000 + 2 * 0x29c;
    m.write32(definition + 0x14, 3); // ammo kind
    m.write32(definition + if flag != 0 { 0x90 } else { 0x8c }, cap);
    m.write16(0x2000 + 3 * 2 + 0x61e, have as u16);
    m.write16(0x3000 + 3 * 2 + 0x38, held as u16);
    with_args(&mut m, &[0x2000, 0x3000, flag, 0]);
    (m, regs())
}
trait W16 {
    fn write16(&mut self, a: u32, v: u16);
}
impl W16 for Mem {
    fn write16(&mut self, a: u32, v: u16) {
        self.write8(a, v as u8);
        self.write8(a + 1, (v >> 8) as u8);
    }
}
#[test]
fn b6db0_cap_boundary_without_callee() {
    for (have, held, cap, flag, ok) in [
        (5, 5, 10, 0, true),
        (4, 5, 10, 0, false),
        (-3, 20, 10, 1, true),
        (-3, 12, 10, 1, false),
    ] {
        let (mut m, mut r) = ammo_fixture(have, held, cap, flag);
        let mut g = Guest {
            results: vec![0],
            ..Guest::default()
        };
        weapons::b6db0(&mut m, &mut g, &mut r);
        assert_eq!(r.eax, ok as u32, "{have} {held} {cap}");
        assert_eq!(g.seen.len(), if ok { 0 } else { 1 });
        assert_eq!((r.esi, r.edi, r.ebx, r.esp), (0x51, 0xd1, 0xb, STACK));
    }
}
#[test]
fn b6db0_fallback_callee_args_and_registers() {
    let (mut m, mut r) = ammo_fixture(0, 0, 10, 7);
    let mut g = Guest {
        results: vec![9],
        ..Guest::default()
    };
    weapons::b6db0(&mut m, &mut g, &mut r);
    assert_eq!(r.eax, 1);
    let (va, regs_in, _) = &g.seen[0];
    assert_eq!(*va, 0x1b6c90);
    assert_eq!((regs_in[0], regs_in[3], regs_in[4]), (10, 7, 0x2000));
    assert_eq!(
        [
            m.read32(regs_in[5]),
            m.read32(regs_in[5] + 4),
            m.read32(regs_in[5] + 8),
            m.read32(regs_in[5] + 12)
        ],
        [0x2000, 0x3000, 1, 7]
    );
}
#[test]
fn b6db0_negative_index_skips_cap_and_returns_callee_truth() {
    let (mut m, mut r) = ammo_fixture(100, 100, 1, 0);
    m.write32(0x3000 + 0x30, 0xffff_ffff);
    let mut g = Guest {
        results: vec![0],
        ..Guest::default()
    };
    weapons::b6db0(&mut m, &mut g, &mut r);
    assert_eq!((r.eax, g.seen.len(), g.seen[0].1[0]), (0, 1, 0xffff_ffff));
}
#[test]
fn b6790_gate_matrix() {
    // (gate 3533F0, check 358560, suppress global) -> forwarded?
    for (gate, check, suppress, forwarded) in [
        (0, 9, 0, true),
        (1, 0, 0, false),
        (1, 4, 0, true),
        (1, 4, 6, false),
        (0, 9, 6, false),
    ] {
        let mut m = Mem::default();
        m.write32(0x6000, 0x1234);
        m.write32(weapons::AMMO_TAKE_SUPPRESS, suppress);
        with_args(&mut m, &[0x6000, 11, 12, 13, 14]);
        let mut r = regs();
        let mut g = Guest {
            results: if gate == 0 {
                vec![0, 77]
            } else {
                vec![gate, check, 77]
            },
            ..Guest::default()
        };
        weapons::b6790(&mut m, &mut g, &mut r);
        let vas: Vec<u32> = g.seen.iter().map(|s| s.0).collect();
        let mut expected = vec![0x3533f0];
        if gate != 0 {
            expected.push(0x358560);
        }
        if forwarded {
            expected.push(0x1b66e0);
        }
        assert_eq!(vas, expected, "{gate} {check} {suppress}");
        assert_eq!((r.esi, r.esp), (0x51, STACK));
        if forwarded {
            let last = g.seen.last().unwrap();
            assert_eq!(
                [m.read32(last.1[5]), m.read32(last.1[5] + 16)],
                [0x6000, 14]
            );
            assert_eq!(r.eax, 77);
        }
        if !forwarded && gate != 0 && check == 0 {
            assert_eq!(r.eax, 0);
        }
        if !forwarded && suppress != 0 {
            assert_eq!(r.eax, suppress);
        }
    }
}
#[test]
fn run_rejects_unlisted_roots() {
    let mut m = Mem::default();
    let mut g = Guest::default();
    let mut r = regs();
    assert!(!weapons::run(0x1bb5c1, &mut m, &mut g, &mut r));
    assert_eq!(
        weapons::WEAPON_ROOTS,
        [0x1b5660, 0x1b6db0, 0x1b6790, 0x1b9a30]
    );
    assert_eq!(weapons::BATCH2_ROOTS, [0x1b7ed0, 0x1c11c0]);
    assert_eq!(weapons::BATCH3_ROOTS, [0x1c0200, 0x1bb5c0]);
    for root in weapons::WEAPON_ROOTS
        .iter()
        .chain(weapons::BATCH2_ROOTS.iter())
        .chain(weapons::BATCH3_ROOTS.iter())
    {
        assert!(
            !ext::EXT_ROOTS.contains(root),
            "{root:x} must stay unadmitted"
        );
    }
}

const PLAYER: u32 = 0x7000;
const DEFINITIONS: u32 = 0x5000;
fn f32_bits(v: f32) -> u32 {
    v.to_bits()
}
/// Unarmed fire-gate fixture: weapon 0x2000 (not type 2), owner 0x2800, entry rate 0.5, 60 frames.
fn b7ed0_fixture(cooldown: u32) -> Mem {
    let mut m = Mem::default();
    m.write32(0x7b0c7c, PLAYER);
    m.write32(PLAYER + 0x14, 0x7100);
    m.write32(0x7100 + 0x7c, 0x7200);
    m.write32(weapons::DEFINITION_TABLE, DEFINITIONS);
    m.write32(0x2000 + 0x3cc, 0x2800);
    m.write32(0x4fa134, 0xffff_ffff); // owner row invalid: not armed
    m.write32(DEFINITIONS + 0x68 + 0x10, f32_bits(0.5)); // entry rate for flag == 0
    m.write32(0x4e8cec, 60);
    m.write32(0x2800 + 0xf0, cooldown);
    m
}
fn b7ed0_run(m: &mut Mem, args: &[u32; 4]) -> ExtRegisters {
    with_args(m, args);
    let mut r = regs();
    weapons::b7ed0(m, &mut r);
    r
}
#[test]
fn b7ed0_cooldown_must_reach_truncated_cost() {
    for (cooldown, expect) in [(29u32, 0u32), (30, 1), (31, 1)] {
        let mut m = b7ed0_fixture(cooldown);
        let r = b7ed0_run(&mut m, &[0x2000, 1, 0, 0]);
        assert_eq!(
            (r.eax, r.ecx, r.esp, r.esi, r.edi, r.ebx),
            (expect, 30, STACK, 0x51, 0xd1, 0xb),
            "{cooldown}"
        );
        assert_eq!(f32::from_bits(r.xmm[0].lanes[0]), 30.0);
    }
}
#[test]
fn b7ed0_mode_must_match_the_token() {
    let mut m = b7ed0_fixture(100);
    assert_eq!(b7ed0_run(&mut m, &[0x2000, 1, 0, 0]).eax, 1);
    assert_eq!(b7ed0_run(&mut m, &[0x2000, 0, 0, 0]).eax, 0);
}
#[test]
fn b7ed0_type2_weapon_with_flag_4000_is_refused_without_touching_xmm() {
    let mut m = b7ed0_fixture(100);
    m.write32(0x2000 + 0x3c, 2);
    m.write32(0x2000 + 0x30, 1);
    m.write32(DEFINITIONS + 0x29c + 0xc, 0x4000);
    let r = b7ed0_run(&mut m, &[0x2000, 1, 0, 0]);
    assert_eq!(
        (r.eax, r.ecx, r.edx, r.xmm[0].lanes),
        (0, 0x29c, 0x7200, [0; 4])
    );
}
#[test]
fn b7ed0_bit7_with_flag_one_refuses() {
    let mut m = b7ed0_fixture(100);
    m.write32(DEFINITIONS + 0x68, 0x80);
    assert_eq!(b7ed0_run(&mut m, &[0x2000, 1, 0, 1]).eax, 0);
    assert_eq!(b7ed0_run(&mut m, &[0x2000, 1, 0, 0]).eax, 1);
}
#[test]
fn b7ed0_cvttss2si_out_of_range_is_x86_indefinite() {
    let mut m = b7ed0_fixture(0);
    m.write32(DEFINITIONS + 0x68 + 0x10, f32_bits(1.0e30));
    let r = b7ed0_run(&mut m, &[0x2000, 1, 0, 0]);
    assert_eq!(r.ecx, 0x8000_0000);
    assert_eq!(r.eax, 1); // cooldown 0 is not below the negative indefinite cost
}
fn c11c0_fixture() -> Mem {
    let mut m = Mem::default();
    m.write32(0x7b0c7c, PLAYER);
    m.write32(PLAYER, 0xabc);
    m.write32(PLAYER + 0x18, 1);
    m.write32(weapons::DEFINITION_TABLE, DEFINITIONS);
    m.write32(0x475c78, f32_bits(1.0));
    m.write32(0x4e8cec, 60);
    m.write32(0x4e794c, f32_bits(1.0));
    m.write32(0x2000 + 0x30, 0xffff_ffff); // record has no definition
    m.write32(DEFINITIONS + 0x68 + 0xc, f32_bits(0.5));
    m
}
fn c11c0_run(m: &mut Mem, guest: &mut Guest) -> ExtRegisters {
    m.write32(STACK, 0x1234);
    let mut r = regs();
    r.eax = 0x2000;
    r.esi = 0x2800;
    weapons::c11c0(m, guest, &mut r);
    r
}
#[test]
fn c11c0_timer_step_clamps_and_calls_42760_with_player_word() {
    let mut m = c11c0_fixture();
    m.write32(0x2800 + 0x108, f32_bits(0.5));
    let mut g = Guest {
        results: vec![1],
        ..Guest::default()
    };
    let r = c11c0_run(&mut m, &mut g);
    assert_eq!(g.seen.iter().map(|s| s.0).collect::<Vec<_>>(), [0x42760]);
    assert_eq!(f32::from_bits(m.read32(0x2000 + 0xbc)), 1.0);
    let timer = f32::from_bits(m.read32(0x2800 + 0x108));
    assert!((timer - (0.5 + 1.0 / 60.0)).abs() < 1e-6, "{timer}");
    assert_eq!(
        (r.esp, r.esi, r.ebx, r.ebp, r.edi),
        (STACK, 0x2800, 0xb, 0xe, 0xd1)
    );
    m.write32(0x2800 + 0x108, f32_bits(0.995));
    m.write32(0x2000 + 0xb8, 0);
    let mut g = Guest {
        results: vec![1],
        ..Guest::default()
    };
    c11c0_run(&mut m, &mut g);
    assert_eq!(f32::from_bits(m.read32(0x2800 + 0x108)), 1.0);
}
#[test]
fn c11c0_unmatched_player_mask_marks_ready() {
    let mut m = c11c0_fixture();
    m.write32(PLAYER + 0x18, 2);
    let mut g = Guest {
        results: vec![1],
        ..Guest::default()
    };
    c11c0_run(&mut m, &mut g);
    assert_eq!(m.read32(0x2000 + 0xb8), 1);
}
#[test]
fn c11c0_counter_reaches_threshold_and_starts_the_shot() {
    let mut m = c11c0_fixture();
    m.write32(0x2000 + 0xb8, 1);
    m.write32(0x2000 + 0xa8, 59);
    m.write32(0x4e7944, 1);
    let mut g = Guest {
        results: vec![1],
        ..Guest::default()
    };
    c11c0_run(&mut m, &mut g);
    assert_eq!(m.read32(0x2000 + 0xa8), 60);
    assert_eq!((m.read32(0x2000 + 0x44), m.read32(0x2000 + 0x48)), (1, 0));
    let vas: Vec<u32> = g.seen.iter().map(|s| s.0).collect();
    assert_eq!(vas, [0x42760, 0x1c0200]);
    // below the threshold (counter 29 + 1 < 60 * 0.5) the shot does not start
    let mut m = c11c0_fixture();
    m.write32(0x2000 + 0xb8, 1);
    m.write32(0x2000 + 0xa8, 28);
    m.write32(0x4e7944, 1);
    let mut g = Guest {
        results: vec![1],
        ..Guest::default()
    };
    c11c0_run(&mut m, &mut g);
    assert_eq!(g.seen.len(), 1);
}

// ---- batch 3: 0x1C0200 shot start (shape checks, the full equivalence is tests/c/test_rust_weapons3.c) ----
const SHOT_RECORD: u32 = 0x2000;
const SHOT_OWNER: u32 = 0x3000;
const SHOT_PLAYER: u32 = 0x4000;
const SHOT_TABLE: u32 = 0x10000;
fn c0200_fixture() -> Mem {
    let mut m = Mem::default();
    m.write32(weapons::DEFINITION_TABLE, SHOT_TABLE);
    m.write32(0x7B0C7C, SHOT_PLAYER);
    m.write32(SHOT_PLAYER + 0x14, 0x4800);
    m.write32(0x4800 + 0x7C, 0x5000);
    m.write32(0x475C78, 0x3f80_0000); // 1.0
    m.write32(0x4E8CEC, 60);
    m.write32(STACK, 0x1234);
    m.write32(STACK + 4, SHOT_OWNER);
    m
}
fn c0200_run(m: &mut Mem, guest: &mut Guest) -> ExtRegisters {
    let mut r = regs();
    r.ebx = SHOT_RECORD;
    weapons::shot::shot_start(m, guest, &mut r);
    r
}
#[test]
fn c0200_aborts_when_the_ready_check_returns_zero() {
    let mut m = c0200_fixture();
    m.write32(SHOT_RECORD + 0x30, 1); // record weapon id
    m.write32(SHOT_PLAYER + 0xE84, 1); // player flag that arms the ready check
    m.write32(SHOT_TABLE + 0x29C, 0); // definition(1) first word is not -1
    let mut g = Guest::default();
    let r = c0200_run(&mut m, &mut g);
    assert_eq!(
        g.seen.iter().map(|s| s.0).collect::<Vec<_>>(),
        vec![0x1b9a30]
    );
    assert_eq!(g.seen[0].1[2], SHOT_RECORD); // EDX = record on the call
    assert_eq!(
        m.read32(SHOT_RECORD + 0x68),
        0,
        "no state reset after an abort"
    );
    assert_eq!(
        (r.ebx, r.ebp, r.esi, r.edi, r.esp),
        (SHOT_RECORD, 0xe, 0x51, 0xd1, STACK)
    );
}
#[test]
fn c0200_weapon_1b_resets_record_reads_recoil_table_and_returns_early() {
    let mut m = c0200_fixture();
    m.write32(SHOT_RECORD + 0x30, 0x1b);
    m.write32(SHOT_RECORD + 0x5C, 0); // fire mode 0
    m.write32(SHOT_TABLE + 0x68 + 0x64, 3); // entry recoil index
    m.write32(0x4ED3C0 + 3 * 8, 0x5555); // recoil table word
    let mut g = Guest::default();
    let r = c0200_run(&mut m, &mut g);
    let calls: Vec<u32> = g.seen.iter().map(|s| s.0).collect();
    assert_eq!(calls.last(), Some(&0x1be7d0), "{calls:x?}");
    assert_eq!(m.read32(SHOT_RECORD + 0x68), 1); // reset marker
    assert_eq!(m.read32(SHOT_RECORD + 0xC4), 0x5555); // recoil value copied from the table
    assert_eq!(m.read32(SHOT_RECORD + 0xAC), 1); // fired flag
    assert_eq!(m.read32(SHOT_RECORD + 0xA0), 0x3f80_0000); // 1.0 spread decay reset
    assert_eq!(
        (r.ebx, r.ebp, r.esi, r.edi, r.esp),
        (SHOT_RECORD, 0xe, 0x51, 0xd1, STACK)
    );
}
#[test]
fn c0200_zero_shots_still_sets_refire_frames_and_takes_ammo() {
    let mut m = c0200_fixture();
    m.write32(SHOT_RECORD + 0x30, 5); // plain weapon id, no player hook
    m.write32(SHOT_RECORD + 0x60, 0x77); // ammo take count word
    m.write32(SHOT_TABLE + 0x68 + 8, 0); // shots per pull = 0: the shot loop is skipped
    m.write32(SHOT_TABLE + 0x298, 0x3f00_0000); // 0.5 * 60 = 30 frames
    m.write32(SHOT_TABLE + 0x68 + 0x54, 0xffff_ffff); // no refire callback
    let mut g = Guest::default();
    let r = c0200_run(&mut m, &mut g);
    let calls: Vec<u32> = g.seen.iter().map(|s| s.0).collect();
    assert_eq!(
        &calls[calls.len() - 2..],
        &[0x1b8530, 0x1b6ff0],
        "{calls:x?}"
    );
    assert_eq!(m.read32(SHOT_OWNER + 0x110), 30);
    assert_eq!(m.read32(SHOT_RECORD + 0xAC), 1);
    assert_eq!(g.seen.last().unwrap().1[4], SHOT_OWNER); // EDI = shooter on the last call
    assert_eq!(
        (r.ebx, r.ebp, r.esi, r.edi, r.esp),
        (SHOT_RECORD, 0xe, 0x51, 0xd1, STACK)
    );
}
#[test]
fn c0200_non_positive_refire_frames_run_the_callback_and_clear_the_field() {
    let mut m = c0200_fixture();
    m.write32(SHOT_RECORD + 0x30, 5);
    m.write32(SHOT_TABLE + 0x68 + 8, 0);
    m.write32(SHOT_TABLE + 0x298, 0); // 0 frames
    m.write32(SHOT_TABLE + 0x68 + 0x54, 3); // refire callback present
    let mut g = Guest::default();
    c0200_run(&mut m, &mut g);
    let calls: Vec<u32> = g.seen.iter().map(|s| s.0).collect();
    assert!(calls.contains(&0x1c0110), "{calls:x?}");
    assert_eq!(m.read32(SHOT_OWNER + 0x110), 0);
}
#[test]
fn c0200_shooter_id_6_fires_counter_plus_one_shots_and_clears_the_counter() {
    let mut m = c0200_fixture();
    m.write32(SHOT_OWNER + 0x30, 6); // shooter id 6 uses the record counter as shot count
    m.write32(SHOT_RECORD + 0x30, 5);
    m.write32(SHOT_RECORD + 0xC0, 4); // counter 4 -> 5 shots
    m.write32(SHOT_TABLE + 6 * 0x29C + 0x68 + 4, 0); // projectile spawn enabled (aux id != -1)
    m.write32(SHOT_TABLE + 6 * 0x29C + 0x68 + 8, 1); // would be one shot without the counter rule
    let mut g = Guest::default();
    c0200_run(&mut m, &mut g);
    let spawns = g.seen.iter().filter(|s| s.0 == 0x1b7680).count();
    assert_eq!(spawns, 5);
    assert_eq!(m.read32(SHOT_RECORD + 0xC0), 0);
}

// ---- batch 3b: 0x1BB5C0 early exits (full equivalence is tests/c/test_rust_weapons_bb5c0.c) ----
const NPC_B: u32 = 0x2000;
const NPC_S: u32 = 0x3000;
fn bb5c0_fixture() -> Mem {
    let mut m = Mem::default();
    m.write32(NPC_B + 0x7C, NPC_S); // controller object
    m.write32(NPC_S + 4, 0x4000); // target object
    m.write32(NPC_B + 0x34, 1); // entity field that must be non-zero
    m.write32(0x4E7944, 1); // tick counter, must be >= 1
    m.write32(NPC_S + 0xA4 + 0x30, 3); // embedded object weapon index (arg1 == 0 selects +0xA4)
    m.write32(STACK, 0x1234);
    m.write32(STACK + 4, NPC_B);
    m.write32(STACK + 8, 0);
    m
}
fn bb5c0_run(m: &mut Mem, guest: &mut Guest) -> ExtRegisters {
    let mut r = regs();
    weapons::bb5c0::bb5c0(m, guest, &mut r);
    r
}
#[test]
fn bb5c0_early_exits_return_zero_without_calls_and_restore_registers() {
    type Break = fn(&mut Mem);
    let breakers: [(&str, Break); 4] = [
        ("target missing", |m| m.write32(NPC_S + 4, 0)),
        ("entity field zero", |m| m.write32(NPC_B + 0x34, 0)),
        ("tick below one", |m| m.write32(0x4E7944, 0)),
        ("weapon sentinel", |m| {
            m.write32(NPC_S + 0xA4 + 0x30, 0xffff_ffff)
        }),
    ];
    for (name, apply) in breakers {
        let mut m = bb5c0_fixture();
        apply(&mut m);
        let mut g = Guest::default();
        let r = bb5c0_run(&mut m, &mut g);
        assert!(g.seen.is_empty(), "{name}");
        assert_eq!(r.eax, 0, "{name}");
        assert_eq!(
            (r.ebx, r.esi, r.edi, r.ebp, r.esp),
            (0xb, 0x51, 0xd1, 0xe, STACK),
            "{name}"
        );
    }
}
#[test]
fn bb5c0_flag_argument_selects_the_second_embedded_object() {
    let mut m = bb5c0_fixture();
    m.write32(STACK + 8, 1); // selects S+0x1D4
    m.write32(NPC_S + 0x1D4 + 0x30, 0xffff_ffff); // sentinel only on the second object: early exit
    let mut g = Guest::default();
    let r = bb5c0_run(&mut m, &mut g);
    assert_eq!(r.eax, 0);
    assert!(g.seen.is_empty());
    let mut m = bb5c0_fixture();
    m.write32(STACK + 8, 1);
    m.write32(NPC_S + 0xA4 + 0x30, 0xffff_ffff); // sentinel on the first object is ignored now
    let mut g = Guest::default();
    bb5c0_run(&mut m, &mut g);
    assert!(
        !g.seen.is_empty(),
        "the second object has a valid weapon index so the shot proceeds"
    );
}
