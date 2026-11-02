// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: replaces=tsfp-ports/c2r/ui_menu
// portreg: wraps=tsfp-gameplay/modes/match_flow
// portreg: wraps=tsfp-gameplay/modes/predicates
// portreg: wraps=tsfp-gameplay/modes/setup
//! T1923: Tier 2 gameplay/modes adapters; bodies in `tsfp-gameplay`, lifted calls, live registers published.
use crate::{
    abi::{ExtRegisters, GuestCall},
    gp_bridge::HostBridge,
    memory::GuestMemory,
    state_wire::Wired,
};
use core::cell::RefCell;

#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-gameplay/src/modes/mod.rs"]
pub(crate) mod modes;

use {modes::ModeHost, tsfp_port_gameplay_modes::return_regs as rr};

impl<M: GuestMemory, G: GuestCall> ModeHost for HostBridge<'_, M, G> {
    fn call(&mut self, target: u32, args: &[u32]) -> u32 {
        self.call_cdecl(target, args)
    }
    fn call_f32(&mut self, target: u32, args: &[u32]) -> f32 {
        self.call_float(target, args)
    }
    fn call_x87_unary(&mut self, target: u32, argument: f32) -> f32 {
        HostBridge::call_x87_unary(self, target, argument)
    }
    fn stack_pointer(&self) -> u32 {
        self.entry_esp()
    }
}

impl<M: GuestMemory, G: GuestCall> crate::ui_menu_capture::PageTickHost for HostBridge<'_, M, G> {
    fn call(&mut self, target: u32, args: &[u32]) -> u32 {
        self.call_cdecl(target, args)
    }
    fn call_f32(&mut self, target: u32, args: &[u32]) -> f32 {
        self.call_float(target, args)
    }
    fn call_x87_unary(&mut self, target: u32, argument: f32) -> f32 {
        HostBridge::call_x87_unary(self, target, argument)
    }
    fn stack_pointer(&self) -> u32 {
        self.entry_esp()
    }
}

impl<H: crate::ui_menu_capture::PageTickHost> ModeHost
    for crate::ui_menu_capture::FinishedPageArg<'_, H>
{
    fn call(&mut self, target: u32, args: &[u32]) -> u32 {
        crate::ui_menu_capture::PageTickHost::call(self, target, args)
    }
    fn call_f32(&mut self, target: u32, args: &[u32]) -> f32 {
        crate::ui_menu_capture::PageTickHost::call_f32(self, target, args)
    }
    fn call_x87_unary(&mut self, target: u32, argument: f32) -> f32 {
        crate::ui_menu_capture::PageTickHost::call_x87_unary(self, target, argument)
    }
    fn stack_pointer(&self) -> u32 {
        crate::ui_menu_capture::PageTickHost::stack_pointer(self)
    }
}

pub fn story_page_tick<M: GuestMemory, G: GuestCall>(
    host: &mut HostBridge<'_, M, G>,
    page: u32,
    entry_ecx: u32,
) {
    let mut adapted = crate::ui_menu_capture::FinishedPageArg::new(host, entry_ecx);
    modes::story_menu::story_page_tick(&mut adapted, page);
}

/// T2135 phase 2: the cell stores of the pure ports (0x67280, 0x68610, ...) go through the GameState owner.
impl<M: GuestMemory, G: GuestCall> ModeHost for Wired<'_, HostBridge<'_, M, G>> {
    fn call(&mut self, target: u32, args: &[u32]) -> u32 {
        ModeHost::call(&mut *self.0, target, args)
    }
    fn call_f32(&mut self, target: u32, args: &[u32]) -> f32 {
        ModeHost::call_f32(&mut *self.0, target, args)
    }
    fn call_x87_unary(&mut self, target: u32, argument: f32) -> f32 {
        ModeHost::call_x87_unary(&mut *self.0, target, argument)
    }
    fn stack_pointer(&self) -> u32 {
        ModeHost::stack_pointer(&*self.0)
    }
}

/// Runtime inputs for the selected Arcade/Challenge descriptor branch owned by T2381.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RuntimeChallengeSetup {
    pub variant: u32,
    pub variant2_participant: Option<i32>,
}

/// Construct the documented mode-6/7 descriptor fields with the Rust-owned model and publish
/// them to the hybrid's existing launch-record wire. The surrounding translated caller retains
/// only its profile, readiness, and roster service edges; this function is the record authority.
pub(crate) fn build_runtime_challenge_setup<M: GuestMemory, G: GuestCall>(
    memory: &mut M,
    guest: &mut G,
    entry: &ExtRegisters,
    descriptor_address: u32,
) -> RuntimeChallengeSetup {
    use modes::challenge_setup::{
        ArcadeCategoryId, ChallengeDescriptor, ChallengeId, ChallengeLaunchRequest,
        ChallengeSetupContext, ChallengeUiMode, LaunchRecord, SelectedChallengeSetup,
    };

    let ui_mode = match memory.read32(0x78ad2c) {
        6 => ChallengeUiMode::Mode6,
        7 => ChallengeUiMode::Mode7,
        mode => panic!("T2381 setup adapter entered for unsupported UI mode {mode}"),
    };
    let variant = memory.read32(descriptor_address.wrapping_add(0x04));
    let level_word = memory.read32(descriptor_address.wrapping_add(0x08));
    let roster = memory.read32(descriptor_address.wrapping_add(0x6c));
    let first_roster_id = memory.read32(roster) as i32;
    let descriptor = ChallengeDescriptor {
        word_04: variant,
        word_08: level_word,
        byte_0c: memory.read8(descriptor_address.wrapping_add(0x0c)),
        word_10: memory.read32(descriptor_address.wrapping_add(0x10)),
        first_roster_id: Some(first_roster_id),
        word_90: memory.read32(descriptor_address.wrapping_add(0x90)),
        word_94: memory.read32(descriptor_address.wrapping_add(0x94)),
        word_98: memory.read32(descriptor_address.wrapping_add(0x98)),
        word_9c: memory.read32(descriptor_address.wrapping_add(0x9c)),
    };
    let mut type_by_variant = [0; 256];
    let variant_byte = variant as u8;
    type_by_variant[usize::from(variant_byte)] =
        memory.read8(0x4d1f7c_u32.wrapping_add(u32::from(variant_byte).wrapping_mul(0x20)));
    let fallback = if variant == 2 && first_roster_id < 0 {
        let cell = RefCell::new(&mut *memory);
        let mut host = HostBridge::new(&cell, guest, entry);
        Some(host.call_cdecl(0x64dd0, &[0, (level_word as u8 as i8 as i32) as u32]) as i32)
    } else {
        None
    };
    let context = ChallengeSetupContext {
        menu_participant_count: memory.read8(0x5236c4),
        current_runtime_mode: memory.read8(0x7de455),
        variant_policy_word_is_zero: memory
            .read32(0x4d1f88_u32.wrapping_add(variant.wrapping_mul(0x20)))
            == 0,
        type_by_variant,
        variant2_fallback_participant: fallback,
        timing_multiplier: memory.read32(0x4e8cec),
    };
    let mut previous_bytes = [0; modes::challenge_setup::LAUNCH_RECORD_BYTES];
    for (offset, byte) in previous_bytes.iter_mut().enumerate() {
        *byte = memory.read8(0x78ad58_u32.wrapping_add(offset as u32));
    }
    let request = ChallengeLaunchRequest {
        category: ArcadeCategoryId(memory.read32(0x78ad38)),
        row: None,
        challenge: ChallengeId(memory.read32(0x78ad40)),
        ui_mode,
    };
    let built = SelectedChallengeSetup::build(
        request,
        descriptor,
        &context,
        &LaunchRecord::from_bytes(previous_bytes),
    )
    .expect("variant-2 fallback service was supplied above");
    for (offset, byte) in built.record.as_bytes().iter().copied().enumerate() {
        memory.write8(0x78ad58_u32.wrapping_add(offset as u32), byte);
    }
    RuntimeChallengeSetup {
        variant,
        variant2_participant: built.variant2_participant.map(|participant| participant.0),
    }
}

fn arg<M: GuestMemory>(m: &M, regs: &ExtRegisters, index: u32) -> u32 {
    m.read32(regs.esp.wrapping_add(4).wrapping_add(4 * index))
}

/// Dispatches a root; `false` when `va` is not one of the modes adapters.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if !ROOTS.contains(&va) {
        return false;
    }
    if va == 0x2d2c70 {
        crate::state_wire::audit_entry(&*m, va);
        if crate::story_sequencer_lane::run_mode_4(m, guest, regs) {
            return true;
        }
        if crate::arcade_setup_runtime::run(va, m, guest, regs) {
            return true;
        }
        // Other menu states (including mode 8 in the real capture corpus) still need
        // their existing mechanically lifted Rust body until their readable branch ports land.
        crate::c2r::ui_menu::game_build_current_setup_record_by_menu_state(m, guest, regs);
        return true;
    }
    let entry = *regs;
    crate::state_wire::audit_entry(&*m, va);
    let (a0, a1, a2) = (arg(m, &entry, 0), arg(m, &entry, 1), arg(m, &entry, 2));
    let cell = RefCell::new(&mut *m);
    let mut bridge = HostBridge::new(&cell, guest, &entry);
    let mut host = Wired(&mut bridge);
    // EAX is only set where the original defines it (the Tier 1 differential names the exceptions).
    let eax = match va {
        0x13dcf0 => Some(modes::predicates::mode_predicate(&host)),
        0x80f40 => Some(modes::predicates::mode_flag_mask_applies(&mut host, a0)),
        0x158c80 => Some(modes::setup::side_tables_stamp_or_setup(&mut host)),
        0x1cd1e0 => {
            let blocked = host.read32(0x7de458) & 0x80_0040 != 0;
            let value = modes::predicates::clear_790958_or_refresh(&mut host);
            rr::apply_edx_if(regs, value, host.0.return_regs().1, !blocked)
        }
        0x69e70 => Some(modes::setup::setup_record_transition_block(&host)),
        0x68610 => Some(modes::match_flow::match_end_pick_next_state(&mut host)),
        0x67280 => Some(tsfp_port_gameplay_modes::return_regs::apply_edx(
            regs,
            modes::setup::mode_setup_load_or_copy_then_start(&mut host),
            host.0.return_regs().1,
        )),
        0x431f0 => Some(modes::setup::transition_set(&mut host, a0, a1, a2)),
        0x2b0af0 => Some(modes::setup::alloc_mode_tables(&mut host)),
        0x233ef0 => {
            modes::setup::setup_count_entries_matching(&mut host);
            None
        }
        _ => return false,
    };
    if let Some(value) = eax {
        regs.eax = value;
    }
    true
}

/// The gameplay/modes lane roots in registration order (appended to `lane_inv::INV_ROOTS`).
pub const ROOTS: [u32; 11] = [
    0x13dcf0, 0x80f40, 0x158c80, 0x1cd1e0, 0x69e70, 0x68610, 0x67280, 0x431f0, 0x2b0af0, 0x233ef0,
    0x2d2c70,
];

/// T2135 phase 3 (Tier 1 style, Rust against Rust, FABRICATED inputs): `Wired` is transparent for the two cell-writing
/// hand ports. The same random guest image and the same scripted callee results are run through the pure port
/// directly and through `Wired` in retire modes 0, 1 and 2. Memory, call trace and EAX must be identical; in mode 1
/// the owner must also agree with the guest cells afterwards. The port against the lifted original stays the T1896
/// differential (`tsfp-c-diff` `modes_t1`, needs the private lift): this test covers only what the wrapper adds.
#[cfg(test)]
mod wired_tests {
    use super::*;
    use crate::state_wire::{MODE_TEST_LOCK, RETIRE, game_state};
    use std::collections::BTreeMap;

    #[derive(Clone)]
    struct Sim {
        ram: BTreeMap<u32, u8>,
        trace: Vec<(u32, Vec<u32>)>,
        rng: u64,
    }
    impl Sim {
        fn next(&mut self) -> u32 {
            self.rng ^= self.rng << 13;
            self.rng ^= self.rng >> 7;
            self.rng ^= self.rng << 17;
            (self.rng >> 16) as u32
        }
    }
    impl GuestMemory for Sim {
        fn read8(&self, a: u32) -> u8 {
            *self.ram.get(&a).unwrap_or(&0)
        }
        fn write8(&mut self, a: u32, v: u8) {
            self.ram.insert(a, v);
        }
    }
    impl ModeHost for Sim {
        fn call(&mut self, target: u32, args: &[u32]) -> u32 {
            self.trace.push((target, args.to_vec()));
            self.next()
        }
        fn call_f32(&mut self, target: u32, args: &[u32]) -> f32 {
            self.trace.push((target, args.to_vec()));
            f32::from_bits(self.next())
        }
        fn call_x87_unary(&mut self, target: u32, argument: f32) -> f32 {
            self.trace.push((target, vec![argument.to_bits()]));
            f32::from_bits(self.next())
        }
        fn stack_pointer(&self) -> u32 {
            0x0012_0000
        }
    }
    impl ModeHost for Wired<'_, Sim> {
        fn call(&mut self, target: u32, args: &[u32]) -> u32 {
            ModeHost::call(&mut *self.0, target, args)
        }
        fn call_f32(&mut self, target: u32, args: &[u32]) -> f32 {
            ModeHost::call_f32(&mut *self.0, target, args)
        }
        fn call_x87_unary(&mut self, target: u32, argument: f32) -> f32 {
            ModeHost::call_x87_unary(&mut *self.0, target, argument)
        }
        fn stack_pointer(&self) -> u32 {
            ModeHost::stack_pointer(&*self.0)
        }
    }

    #[derive(Default)]
    struct Guest(Vec<u32>);
    impl GuestCall for Guest {
        fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
            self.0.push(va);
            regs.eax = 23;
        }
    }

    fn seeded(seed: u64) -> Sim {
        let mut sim = Sim {
            ram: BTreeMap::new(),
            trace: vec![],
            rng: seed | 1,
        };
        // mode block and working copy: random words, a quarter of them zero
        for base in (0x7de300u32..0x7de4a0).step_by(4) {
            let word = if sim.next() % 4 == 0 { 0 } else { sim.next() };
            sim.write32(base, word);
        }
        // bias the branch-selecting words: match state 0..8, mode 0..13, the flag bit that gates the setup path
        let (m, n) = (sim.next() % 9, sim.next() % 14);
        sim.write32(0x7de30c, m);
        sim.write8(0x7de455, n as u8);
        let flags = sim.next() | if sim.next() % 2 == 0 { 0x80_0000 } else { 0 };
        sim.write32(0x7de398, flags);
        let level = sim.next();
        sim.write32(0x79094c, level);
        sim
    }

    fn run(sim: &mut Sim, va: u32, wired: bool) -> u32 {
        match (va, wired) {
            (0x67280, false) => modes::setup::mode_setup_load_or_copy_then_start(sim),
            (_, false) => modes::match_flow::match_end_pick_next_state(sim),
            (0x67280, true) => modes::setup::mode_setup_load_or_copy_then_start(&mut Wired(sim)),
            (_, true) => modes::match_flow::match_end_pick_next_state(&mut Wired(sim)),
        }
    }

    fn setup_sim(variant: u32, roster_id: i32) -> (Sim, Guest) {
        let mut sim = Sim {
            ram: BTreeMap::new(),
            trace: vec![],
            rng: 1,
        };
        let mut guest = Guest::default();
        sim.write32(0x78ad2c, if variant == 2 { 6 } else { 7 });
        sim.write32(0x78ad38, 4);
        sim.write32(0x78ad40, 0x1234);
        sim.write8(0x5236c4, 3);
        sim.write8(0x7de455, 0x0a);
        sim.write8(0x4d1f7c + variant.wrapping_mul(0x20), 9);
        sim.write32(0x4e8cec, 5);
        sim.write32(0x4d1f88 + variant.wrapping_mul(0x20), 0);
        sim.write32(0x8000 + 0x04, variant);
        sim.write32(0x8000 + 0x08, 2);
        sim.write8(0x8000 + 0x0c, 0x33);
        sim.write32(0x8000 + 0x10, 0x40);
        sim.write32(0x8000 + 0x6c, 0x9000);
        sim.write32(0x9000, roster_id as u32);
        sim.write32(0x8000 + 0x90, 4);
        sim.write32(0x8000 + 0x94, 0x94);
        sim.write32(0x8000 + 0x98, 0x98);
        sim.write32(0x8000 + 0x9c, 0x9c);
        for offset in 0..modes::challenge_setup::LAUNCH_RECORD_BYTES as u32 {
            sim.write8(0x78ad58 + offset, 0xaa);
        }
        // Keep the guest separate from the memory object passed to the host bridge.
        guest.0.clear();
        (sim, guest)
    }

    fn zero_registers() -> ExtRegisters {
        ExtRegisters {
            eax: 0,
            ecx: 0,
            edx: 0,
            esp: 0x0012_0000,
            ebx: 0,
            ebp: 0,
            esi: 0,
            edi: 0,
            eflags: 0,
            fpu: crate::abi::Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0,
            },
            xmm: [crate::abi::Xmm { lanes: [0; 4] }; 5],
            xmm_hi: [crate::abi::Xmm { lanes: [0; 4] }; 3],
        }
    }

    #[test]
    fn runtime_adapter_publishes_owned_fields_and_preserves_unowned_bytes() {
        let mut sim = setup_sim(3, 5).0;
        let mut guest = Guest::default();
        let result = build_runtime_challenge_setup(&mut sim, &mut guest, &zero_registers(), 0x8000);
        assert_eq!(result.variant, 3);
        assert_eq!(result.variant2_participant, None);
        assert_eq!(sim.read8(0x78ad58), 3);
        assert_eq!(sim.read8(0x78adac), 2);
        assert_eq!(sim.read8(0x78adad), 9);
        assert_eq!(sim.read8(0x78adae), 3);
        assert_eq!(sim.read32(0x78adb0), 0x0208_0040);
        assert_eq!(sim.read32(0x78adb8), 20);
        assert_eq!(sim.read32(0x78adc8), 0x1234);
        assert_eq!(sim.read32(0x78ade4), 1.0f32.to_bits());
        assert_eq!(
            sim.read8(0x78ad78),
            0xaa,
            "unlisted record bytes retain their value"
        );
        assert!(guest.0.is_empty());
    }

    #[test]
    fn runtime_adapter_routes_missing_variant_two_participant_to_service() {
        let mut sim = setup_sim(2, -1).0;
        let mut guest = Guest::default();
        let result = build_runtime_challenge_setup(&mut sim, &mut guest, &zero_registers(), 0x8000);
        assert_eq!(result.variant, 2);
        assert_eq!(result.variant2_participant, Some(23));
        assert_eq!(guest.0, [0x64dd0]);
    }

    #[test]
    fn wired_is_transparent_for_67280_and_68610() {
        let _guard = MODE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut cases = 0u32;
        for retire_mode in [0u8, 1, 2] {
            for va in [0x67280u32, 0x68610] {
                for seed in 0..2000u64 {
                    let base = seeded(
                        seed.wrapping_mul(0x9e37_79b9_7f4a_7c15)
                            .wrapping_add(va as u64),
                    );
                    let (mut plain, mut wired) = (base.clone(), base.clone());
                    RETIRE.reset();
                    RETIRE.set_mode(retire_mode);
                    let (e0, e1) = (run(&mut plain, va, false), run(&mut wired, va, true));
                    assert_eq!(e0, e1, "eax va {va:x} seed {seed} mode {retire_mode}");
                    assert_eq!(plain.trace, wired.trace, "trace va {va:x} seed {seed}");
                    assert_eq!(
                        plain.ram, wired.ram,
                        "memory va {va:x} seed {seed} mode {retire_mode}"
                    );
                    if retire_mode != 0 && RETIRE.is_init() {
                        assert_eq!(
                            RETIRE.check(&wired),
                            None,
                            "owner/guest va {va:x} seed {seed} mode {retire_mode}"
                        );
                    }
                    cases += 1;
                }
            }
        }
        RETIRE.reset();
        RETIRE.set_mode(game_state::RETIRE_OFF);
        assert_eq!(cases, 12000);
    }
}
