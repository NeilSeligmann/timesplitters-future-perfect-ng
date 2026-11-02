// SPDX-License-Identifier: GPL-3.0-or-later
//! T2138 (S3 Retire pilot 2): the persistent PlayerSet owner behind the wired player writers and the converted readers.
//! T3610: the owner, its modes, the setters/readers (`store`, `get8..32`, `Wired`, `audit_entry`) moved down to
//! `tsfp-port-wire::player_wire` (re-exported here, so `crate::player_wire::*` is unchanged and the C2R batches that
//! use it can live in family crates). This file keeps what calls back into the facade lanes (`run_weapon`,
//! `run_scene`) and the root tables that `tools.t2138_player_attr`, `tools.s3_unwired_share` and the lane tests parse.
//! Design, modes, the re-base rule and the measured drift: docs/t-s3-retire-player.md.
#[cfg(test)]
use crate::leaf_wire::player::{PTR_RECORD, PTR_STATS, Slot};
pub use crate::memory::GuestMemory;
#[allow(unused_imports)]
use crate::state_wire::game_state::{RETIRE_AUDIT, RETIRE_OWN};
pub use tsfp_port_wire::player_wire::*;

/// Lane roots whose Rust translation stores into the record or the stats table (27 of `player_wired`, 7 of
/// `player2_wired` and the two stats writers `0x238510 0x2385B0` of `leaf_wired`; T2118 adds hand-writer roots `0x1C0200` and `0x1B7BC0`).
/// T2138 follow-up includes all eleven selectable player3 roots and two existing object-writer wrappers.
/// A lifted run of any of them writes the record behind the owner.
pub const REQUIRED_ROOTS: [u32; 56] = [
    0xd5eb0, 0x25a0c0, 0x25c120, 0x25a7f0, 0x1c3100, 0x25d520, 0x259e50, 0xcf8d0, 0x1c4400,
    0xd2b80, 0x258f50, 0x259930, 0xd1e20, 0x1bf410, 0xd2190, 0x1bc890, 0x1c2e80, 0xd2280, 0xa0d90,
    0xd0730, 0x25a080, 0x25c650, 0xd01c0, 0x25e650, 0x25a490, 0xd4100, 0x25b720, 0x259df0,
    0x1b7680, 0x1bdfd0, 0x1bd820, 0x219ca0, 0x1bdaf0, 0x1c2090, 0x1c1c50, 0x238510, 0x2385b0,
    // T2138 remote follow-up: the selectable caller-pointer writers added by T2137 pass 3.
    0x88950, 0xcf510, 0x151e80, 0x1baee0, 0x153cd0, 0x1b9dc0, 0x1b7c50, 0x1c2c00, 0x1bc990,
    0x1b7bc0, 0x1c0200, 0x1bf9e0,
    // Existing object-writer roots now also wrap PlayerSet reads and writes.
    0xe1200, 0x1ba130,
    // These player3 roots already dispatch through the weapons lane.
    0x1c2af0, 0x1b8950,
    // T2118 pass 3: renderer camera writers of the record facing words (+0xC0..+0xC8).
    0xcbd10, 0xccab0,
    // T2118 pass 4: animation start, the layer blocks inside the record (+0xE94..+0xF2C).
    0x5a360,
];

/// Existing roots whose retained logic runs through the readable authority wrapper.
/// These are adapters, not new port registrations. Callees keep their own selection.
pub const WRAPPER_ROOTS: [u32; 2] = [0xe1200, 0x1ba130];

/// T2118: existing weapon writers, adapted without registering a second port.
pub const WEAPON_WRAPPER_ROOTS: [u32; 3] = [0x1b7bc0, 0x1c0200, 0x1bf9e0];

/// Selected weapon bodies retain their ABI logic behind the typed authority boundary.
/// Unknown addresses leave memory and registers untouched.
pub fn run_weapon<M: GuestMemory, C: crate::abi::GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut crate::abi::ExtRegisters,
) -> bool {
    if !WEAPON_WRAPPER_ROOTS.contains(&va) {
        return false;
    }
    audit_entry(m, va);
    let mut owned = Wired(m);
    match va {
        0x1b7bc0 => crate::weapons_slice::run(va, true, &mut owned, guest, regs),
        0x1c0200 => crate::weapons::run(va, &mut owned, guest, regs),
        0x1bf9e0 if crate::weapon_hands_runtime::enabled() => {
            crate::weapon_hands_runtime::dispatch_nested(va, &mut owned, guest, regs)
        }
        0x1bf9e0 => crate::c2r::gameplay_inventory::run(va, &mut owned, guest, regs),
        _ => unreachable!(),
    }
}

/// T2118 pass 3: renderer camera writers (scene lane) that store the record facing words `+0xC0..+0xC8`.
/// Adapters over the existing `scene_batch` translations, not new port registrations.
pub const SCENE_WRAPPER_ROOTS: [u32; 2] = [0xcbd10, 0xccab0];

/// Selected camera bodies run behind the typed authority boundary (owner and mirror for record
/// stores, retire-policy reads). Off mode is a plain pass-through. Unknown addresses do nothing.
pub fn run_scene<M: GuestMemory, C: crate::abi::GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut crate::abi::ExtRegisters,
) -> bool {
    if !SCENE_WRAPPER_ROOTS.contains(&va) {
        return false;
    }
    audit_entry(m, va);
    crate::render_lane2::scene_batch::run(va, &mut Wired(m), guest, regs)
}

/// T2118 pass 4: the animation start writer (anim lane, `anim_lane2`), whose layer blocks can lie in the record.
pub const ANIM_WRAPPER_ROOTS: [u32; 1] = [0x5a360];

/// In an active mode (own, audit) the readable `start` port is authoritative for `0x5A360` and its stores go through
/// `Wired`; off keeps the shadow comparison against the retained original. Runs the entry hook when active.
pub fn anim_start_authoritative<M: GuestMemory>(m: &M) -> bool {
    tsfp_port_wire::player_wire::anim_start_authoritative_at(m, ANIM_WRAPPER_ROOTS[0])
}

/// Host option entry (`--retire-player off|own|audit`, runtime.rs `tsfp_rust_retire_player_configure`). Mode 1 is refused
/// (`Err(first missing root)`) unless every required root is selected; audit only observes and is always allowed.
pub fn configure(mode: u8, is_selected: impl Fn(u32) -> bool) -> Result<(), u32> {
    tsfp_port_wire::player_wire::configure_roots(mode, &REQUIRED_ROOTS, is_selected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::leaf_wire::player::{CONTROLLED_CELL, RECORD_CELL, STATS_CELL};
    use crate::mem::GuestAccess;

    struct Ram(Vec<u8>);
    impl GuestMemory for Ram {
        fn read8(&self, a: u32) -> u8 {
            self.0[a as usize]
        }
        fn write8(&mut self, a: u32, v: u8) {
            self.0[a as usize] = v;
        }
    }
    fn image() -> Ram {
        let mut m = Ram(vec![0; 0x80_0000]);
        m.write32(RECORD_CELL, 0x10_0000);
        m.write32(CONTROLLED_CELL, 0x10_0000);
        m.write32(STATS_CELL, 0x14_0000);
        m
    }

    #[test]
    fn required_roots_are_distinct() {
        let mut roots = REQUIRED_ROOTS.to_vec();
        roots.sort_unstable();
        roots.dedup();
        assert_eq!(roots.len(), REQUIRED_ROOTS.len());
    }

    #[test]
    fn guard_covers_every_selectable_wired_player_body() {
        let is_selectable = |va: &u32| {
            crate::lane_inv::INV_ROOTS.contains(va)
                || crate::weapons_lane::WEAPON_ROOTS.contains(va)
                || crate::weapons_lane::PLAYER_ROOTS.contains(va)
                || crate::scene_lane::ROOTS.contains(va)
                || crate::anim_lane2::ROOTS.contains(va)
        };
        let mut expected: Vec<u32> = crate::c2r::player_wired::ROOTS
            .iter()
            .chain(crate::c2r::player2_wired::ROOTS.iter())
            .chain(crate::c2r::player3_wired::ROOTS.iter())
            .chain(WRAPPER_ROOTS.iter())
            .chain(WEAPON_WRAPPER_ROOTS.iter())
            .chain(SCENE_WRAPPER_ROOTS.iter())
            .chain(ANIM_WRAPPER_ROOTS.iter())
            .chain([0x238510, 0x2385b0].iter())
            .copied()
            .filter(is_selectable)
            .collect();
        expected.sort_unstable();
        expected.dedup();
        let mut actual = REQUIRED_ROOTS.to_vec();
        actual.sort_unstable();
        assert_eq!(actual, expected);
    }

    #[test]
    fn own_needs_every_required_root() {
        for hole in REQUIRED_ROOTS {
            assert_eq!(configure(RETIRE_OWN, |va| va != hole), Err(hole));
        }
        assert_eq!(PLAYER.mode(), 0);
    }

    #[test]
    fn wrapper_uses_owned_reads_and_routes_straddling_stores() {
        let mut m = image();
        m.write32(0x10_0700, 0x1122_3344);
        m.write32(0x10_0704, 0x5566_7788);
        PLAYER.set_mode(RETIRE_OWN);
        PLAYER.ensure_init(&m);
        m.write32(0x10_0700, 0);
        m.write32(0x10_0704, 0);
        let mut wired = Wired(&mut m);
        assert_eq!(wired.read32(0x10_0701), 0x8811_2233);
        assert_eq!(wired.read16(0x10_0703), 0x8811);
        wired.write32(0x10_0703, 0xaabb_ccdd);
        assert_eq!(wired.read32(0x10_0700), 0xdd22_3344);
        assert_eq!(wired.read32(0x10_0704), 0x55aa_bbcc);
        // Partial stores merge with the owner, preserving its untouched bytes.
        assert_eq!(wired.0.read32(0x10_0700), 0xdd22_3344);
        assert_eq!(wired.0.read32(0x10_0704), 0x55aa_bbcc);
        // An unowned first byte must not bypass the owned bytes that follow it.
        wired.write32(0x0f_ffff, 0x1234_5678);
        assert_eq!(wired.0.read8(0x0f_ffff), 0x78);
        assert_eq!(PLAYER.word(Slot::Record(0)) & 0x00ff_ffff, 0x0012_3456);
        // The last byte of the record is owned, the three trailing bytes are raw.
        wired.write32(0x10_1fff, 0x89ab_cdef);
        assert_eq!(wired.read32(0x10_1fff), 0x89ab_cdef);
        assert_eq!(PLAYER.word(Slot::Record(2047)) >> 24, 0xef);
    }

    #[test]
    fn wrapper_differential_against_plain_guest_memory() {
        // Tier 1 style: 2,000 FABRICATED operations per mode, no lifted body or
        // real route. The reference is the plain GuestMemory byte contract.
        for mode in [0, RETIRE_AUDIT, RETIRE_OWN] {
            PLAYER.reset();
            PLAYER.set_mode(mode);
            let mut actual = image();
            let mut reference = image();
            let mut seed = 0x2138_u32;
            for n in 0..2000u32 {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let base = match n % 4 {
                    0 => 0x10_0000,
                    1 => 0x14_0000,
                    2 => 0x18_0000,
                    _ => 0x0f_fffc,
                };
                let address = base + (seed & 0x3ff);
                let value = seed.rotate_left(13);
                let mut wired = Wired(&mut actual);
                match n % 3 {
                    0 => {
                        wired.write8(address, value as u8);
                        reference.write8(address, value as u8);
                    }
                    1 => {
                        wired.write16(address, value as u16);
                        reference.write16(address, value as u16);
                    }
                    _ => {
                        wired.write32(address, value);
                        reference.write32(address, value);
                    }
                }
                assert_eq!(wired.read8(address), reference.read8(address));
                assert_eq!(wired.read16(address), reference.read16(address));
                assert_eq!(wired.read32(address), reference.read32(address));
            }
            assert_eq!(actual.0, reference.0);
            if mode == 0 {
                assert!(!PLAYER.is_init());
            } else {
                assert_eq!(PLAYER.scan(&actual, false, |_, _, _| {}), 0);
            }
        }
    }

    #[test]
    fn wrapper_roots_preserve_retained_bodies_on_seeded_inputs() {
        use crate::abi::{ExtRegisters, GuestCall};
        struct RandomCalls {
            seed: u32,
            calls: Vec<u32>,
        }
        impl GuestCall for RandomCalls {
            fn call(&mut self, va: u32, r: &mut ExtRegisters) {
                self.calls.push(va);
                self.seed = self.seed.wrapping_mul(1664525).wrapping_add(1013904223);
                r.eax = self.seed;
            }
        }
        // Tier 1 style contract differential against retained Rust translations,
        // FABRICATED fixtures and RNG stubs; not a new lifted-C or route proof.
        for mode in [0, RETIRE_AUDIT, RETIRE_OWN] {
            for va in WRAPPER_ROOTS {
                for seed in 0..1000u32 {
                    PLAYER.reset();
                    PLAYER.set_mode(mode);
                    let mut reference = image();
                    let stack = 0x18_0100;
                    let base = if seed % 2 == 0 { 0x10_0700 } else { 0x16_0000 };
                    reference.write32(stack + 4, base);
                    if va == 0xe1200 {
                        reference.write32(stack + 8, (0.25f32).to_bits());
                        reference.write32(0x475c78, 1f32.to_bits());
                        reference.write32(0x4e8cec, 1);
                        reference.write32(0x4e794c, 1f32.to_bits());
                        reference.write32(0x4785d0, (1.0f32 / 4294967296.0).to_bits());
                        reference.write32(0x475ccc, 4294967296f32.to_bits());
                        for off in (0..=0x18).step_by(4) {
                            reference.write32(base + off, ((seed % 17) as f32 / 16.0).to_bits());
                        }
                    } else {
                        let slot = seed % 8;
                        reference.write32(stack + 8, slot);
                        reference.write32(base + 8, seed % 2);
                        reference.write32(0x790950, 1);
                        reference.write32(0x4fa160 + slot * 0x3c, seed % 16);
                    }
                    let mut actual = Ram(reference.0.clone());
                    let mut expected_regs: ExtRegisters = unsafe { core::mem::zeroed() };
                    expected_regs.esp = stack;
                    expected_regs.ebx = seed.rotate_left(7);
                    let mut actual_regs = expected_regs;
                    let mut expected_calls = RandomCalls {
                        seed,
                        calls: Vec::new(),
                    };
                    let mut actual_calls = RandomCalls {
                        seed,
                        calls: Vec::new(),
                    };
                    if va == 0xe1200 {
                        assert!(crate::c2r::objchar_wired2::run(
                            va,
                            &mut reference,
                            &mut expected_calls,
                            &mut expected_regs
                        ));
                    } else {
                        assert!(crate::c2r::objchar_wired3::run(
                            va,
                            &mut reference,
                            &mut expected_calls,
                            &mut expected_regs
                        ));
                    }
                    assert!(crate::leaf_lane::run(
                        va,
                        &mut actual,
                        &mut actual_calls,
                        &mut actual_regs
                    ));
                    assert_eq!(
                        actual_regs, expected_regs,
                        "mode {mode}, root {va:x}, seed {seed}"
                    );
                    assert_eq!(actual_calls.calls, expected_calls.calls);
                    assert_eq!(
                        actual.0, reference.0,
                        "mode {mode}, root {va:x}, seed {seed}"
                    );
                    if mode == 0 {
                        assert!(!PLAYER.is_init());
                    } else {
                        assert_eq!(PLAYER.scan(&actual, false, |_, _, _| {}), 0);
                    }
                }
            }
        }
        PLAYER.set_mode(0);
        PLAYER.reset();
    }

    #[test]
    fn t2376_slot_stash_wrapper_matches_translation() {
        use crate::abi::{ExtRegisters, GuestCall};
        struct Calls;
        impl GuestCall for Calls {
            fn call(&mut self, _va: u32, r: &mut ExtRegisters) {
                r.eax = 0;
            }
        }
        // Hands disabled: the wrapper must still run the retained body behind the owner.
        PLAYER.reset();
        PLAYER.set_mode(RETIRE_OWN);
        let mut wrapped = image();
        let stack = 0x18_0100;
        wrapped.write32(stack + 4, 3);
        let mut direct = Ram(wrapped.0.clone());
        let mut wrapped_regs: ExtRegisters = unsafe { core::mem::zeroed() };
        wrapped_regs.esp = stack;
        wrapped_regs.ecx = 0x10_0000;
        let mut direct_regs = wrapped_regs;
        assert!(run_weapon(
            0x1bf9e0,
            &mut wrapped,
            &mut Calls,
            &mut wrapped_regs
        ));
        assert!(crate::c2r::gameplay_inventory::run(
            0x1bf9e0,
            &mut direct,
            &mut Calls,
            &mut direct_regs
        ));
        assert_eq!(wrapped.0, direct.0);
        PLAYER.set_mode(0);
        PLAYER.reset();
    }

    #[test]
    fn weapon_writers_preserve_abi_and_update_owned_words() {
        use crate::abi::{ExtRegisters, GuestCall};
        struct Calls(Vec<u32>);
        impl GuestCall for Calls {
            fn call(&mut self, va: u32, r: &mut ExtRegisters) {
                self.0.push(va);
                r.eax = 0; // shot readiness abort, after the id-6 burst reset
            }
        }
        for mode in [0, RETIRE_AUDIT, RETIRE_OWN] {
            // 0x1bf9e0 is the slot stash, not a shot writer: this fixture's shot reference
            // does not apply to it (see t2376_slot_stash_wrapper_matches_translation).
            for va in WEAPON_WRAPPER_ROOTS
                .into_iter()
                .filter(|va| *va != 0x1bf9e0)
            {
                for seed in 0..2000u32 {
                    PLAYER.reset();
                    PLAYER.set_mode(mode);
                    let mut reference = image();
                    let stack = 0x18_0100;
                    let record = 0x10_0000;
                    let weapon = if seed % 2 == 0 {
                        record + 0x700
                    } else {
                        0x16_0000
                    };
                    let shooter = record + 0x1200;
                    // Seed every owned word before initialization: a bypassed setter
                    // cannot pass merely because both snapshots started at zero.
                    for off in (0..0x2000).step_by(4) {
                        reference
                            .write32(record + off, seed.wrapping_mul(1664525) ^ off ^ 0xa5a5_5a5a);
                    }
                    if va == 0x1b7bc0 {
                        reference.write32(stack + 4, seed);
                        reference.write32(stack + 8, weapon);
                        reference.write32(stack + 12, shooter);
                        reference.write32(stack + 16, seed.rotate_left(9));
                    } else {
                        reference.write32(stack + 4, shooter);
                        reference.write32(weapon + 0x5c, 0);
                        reference.write32(weapon + 0x30, 1);
                        reference.write32(weapon + 0xc0, seed % 7);
                        reference.write32(shooter + 0x30, 6);
                        reference.write32(record + 4, 0);
                        reference.write32(record + 0x14, 0x17_0000);
                        reference.write32(0x17_007c, 0);
                        reference.write32(record + 0xe84, 1);
                        reference.write32(0x4f9bac, 0x15_0000);
                    }
                    let mut actual = Ram(reference.0.clone());
                    let mut expected_regs: ExtRegisters = unsafe { core::mem::zeroed() };
                    expected_regs.esp = stack;
                    expected_regs.ebx = weapon;
                    expected_regs.esi = seed;
                    expected_regs.edi = seed.rotate_left(7);
                    let mut actual_regs = expected_regs;
                    let mut expected_calls = Calls(Vec::new());
                    let mut actual_calls = Calls(Vec::new());
                    if mode != 0 {
                        PLAYER.ensure_init(&actual);
                    }
                    if va == 0x1b7bc0 {
                        crate::weapons_slice::run(
                            va,
                            true,
                            &mut reference,
                            &mut expected_calls,
                            &mut expected_regs,
                        );
                    } else {
                        crate::weapons::shot::shot_start(
                            &mut reference,
                            &mut expected_calls,
                            &mut expected_regs,
                        );
                    }
                    assert!(crate::leaf_lane::run(
                        va,
                        &mut actual,
                        &mut actual_calls,
                        &mut actual_regs
                    ));
                    assert_eq!(
                        actual_regs, expected_regs,
                        "mode {mode}, root {va:x}, seed {seed}"
                    );
                    assert_eq!(actual_calls.0, expected_calls.0);
                    assert_eq!(actual.0, reference.0);
                    if mode == 0 {
                        assert!(!PLAYER.is_init());
                    } else {
                        // Include pass-through words: they still have write-through
                        // setter obligations even though reads retain guest authority.
                        for off in (0..0x2000).step_by(4) {
                            let slot = Slot::Record(off as usize / 4);
                            assert_eq!(
                                PLAYER.word(slot),
                                actual.read32(record + off),
                                "setter mode {mode}, root {va:x}, seed {seed}, offset {off:x}"
                            );
                        }
                    }
                }
            }
        }
        PLAYER.set_mode(0);
        PLAYER.reset();
    }

    /// T2118 pass 4: the animation start `0x5A360` in an active mode runs the readable port through `Wired`: the image and
    /// callees equal the port on plain memory, and every owned record word is mirrored. FABRICATED fixtures (clip table,
    /// slot map, policy, layer blocks inside the record), Tier 1 host-free. Off mode keeps the shadow path (not run here).
    #[test]
    fn anim_start_writer_updates_owned_words() {
        use crate::abi::{ExtRegisters, GuestCall};
        struct Calls(Vec<u32>);
        impl GuestCall for Calls {
            fn call(&mut self, va: u32, r: &mut ExtRegisters) {
                self.0.push(va);
                r.eax = 0;
            }
        }
        let mut changed = 0usize;
        for mode in [RETIRE_AUDIT, RETIRE_OWN] {
            for seed in 0..2000u32 {
                PLAYER.reset();
                PLAYER.set_mode(mode);
                let mut reference = image();
                let record = 0x10_0000u32;
                let (current, previous, request) = (record + 0xe94, record + 0xeec, record + 0xf44);
                let stack = 0x18_0100;
                let word = |off: u32| seed.wrapping_mul(1664525) ^ off.wrapping_mul(2654435761);
                for off in (0..0x2000).step_by(4) {
                    reference.write32(record + off, word(off) & 0x3fff_ffff);
                }
                // previous plays id 0, 1 or 2 so both the cross-fade copy and the plain start are reached
                reference.write32(previous, seed % 3);
                let (id, clip) = (1 + seed % 4, 0x16_0000);
                reference.write32(0x7e3954, 0x15_0000);
                for slot in 0..8u32 {
                    reference.write32(0x15_0000 + 4 * slot, if seed % 7 == 0 { 0 } else { clip });
                    reference.write32(
                        0x7de660 + 8 * slot,
                        if seed % 5 == 0 { 0x201 } else { seed & 0x1ff },
                    );
                }
                for i in 0..8u32 {
                    reference.write8(0x7e03c0 + 2 * i, i as u8);
                }
                reference.write32(clip + 0x18, (1.0 + (seed % 9) as f32).to_bits());
                let blend = if seed % 2 == 0 { 0.0f32 } else { 0.25 };
                for (i, value) in [
                    id,
                    blend.to_bits(),
                    1.5f32.to_bits(),
                    1f32.to_bits(),
                    request,
                    current,
                    previous,
                ]
                .into_iter()
                .enumerate()
                {
                    reference.write32(stack + 4 + 4 * i as u32, value);
                }
                let mut actual = Ram(reference.0.clone());
                let mut regs: ExtRegisters = unsafe { core::mem::zeroed() };
                regs.esp = stack;
                let mut actual_regs = regs;
                let (mut expected_calls, mut actual_calls) = (Calls(Vec::new()), Calls(Vec::new()));
                PLAYER.ensure_init(&actual);
                let before: Vec<u32> = (0..0x2000)
                    .step_by(4)
                    .map(|off| reference.read32(record + off))
                    .collect();
                crate::anim_lane2::start_port(
                    &mut reference,
                    &mut expected_calls,
                    &regs,
                    request,
                    current,
                    previous,
                );
                assert!(crate::anim_lane2::run(
                    0x5a360,
                    &mut actual,
                    &mut actual_calls,
                    &mut actual_regs
                ));
                assert_eq!(actual_calls.0, expected_calls.0, "mode {mode}, seed {seed}");
                assert_eq!(actual.0, reference.0, "mode {mode}, seed {seed}");
                for (index, off) in (0..0x2000u32).step_by(4).enumerate() {
                    let guest = actual.read32(record + off);
                    changed += usize::from(guest != before[index]);
                    assert_eq!(
                        PLAYER.word(Slot::Record(index)),
                        guest,
                        "setter mode {mode}, seed {seed}, offset {off:x}"
                    );
                }
            }
        }
        assert!(
            changed > 20_000,
            "non-vacuous: {changed} changed record words"
        );
        PLAYER.set_mode(0);
        PLAYER.reset();
    }

    /// T2118 pass 3: the camera writers through the authority adapter equal the plain
    /// translation (image, registers, callees) and leave every owned record word mirrored.
    /// FABRICATED fixtures (seeded record, camera object and stack arguments), Tier 1 host-free.
    #[test]
    fn scene_writers_preserve_abi_and_update_owned_words() {
        use crate::abi::{ExtRegisters, GuestCall};
        struct Calls(Vec<u32>, u32);
        impl GuestCall for Calls {
            fn call(&mut self, va: u32, r: &mut ExtRegisters) {
                self.0.push(va);
                // 0x23EF10 (path lookup) answers -1 on even seeds: the explicit target-vector path.
                r.eax = if va == 0x23ef10 && self.1 % 2 == 0 {
                    u32::MAX
                } else {
                    0
                };
            }
        }
        let mut record_writes = [0usize; 2];
        for mode in [0, RETIRE_AUDIT, RETIRE_OWN] {
            for va in SCENE_WRAPPER_ROOTS {
                for seed in 0..2000u32 {
                    PLAYER.reset();
                    PLAYER.set_mode(mode);
                    let mut reference = image();
                    let record = 0x10_0000;
                    let stack = 0x18_0100;
                    let word = |off: u32| {
                        seed.wrapping_mul(1664525) ^ off.wrapping_mul(2654435761) ^ 0x3c00_0000
                    };
                    for off in (0..0x2000).step_by(4) {
                        reference.write32(record + off, word(off) & 0x3fff_ffff);
                        reference.write32(0x12_0000 + off, word(off ^ 0x55) & 0x3fff_ffff);
                        reference.write32(0x13_0000 + off, word(off ^ 0xaa) & 0x3fff_ffff);
                    }
                    reference.write32(record + 0x14, 0x17_0000);
                    reference.write32(0x7b0cc0, 0x12_0000);
                    reference.write32(stack + 4, seed);
                    reference.write32(stack + 8, 0x13_0000);
                    reference.write32(stack + 0xc, 0x13_0400);
                    reference.write32(stack + 0x10, 0x13_0800);
                    reference.write32(stack + 0x14, (seed as f32).to_bits());
                    for cell in [0x475c78u32, 0x478900, 0x4785cc, 0x4e794c] {
                        reference.write32(cell, 1f32.to_bits());
                    }
                    reference.write32(0x4e8cec, 1 + seed % 3);
                    // 0xCCAB0 dispatches the camera state `camera+0x218` (when `[0x7B0CE8]` is outside 4..=9) through a byte index table and a
                    // dword target table. FABRICATED single-entry tables pick the record-facing
                    // writer case (even seeds) or the record-reading follow case (odd seeds).
                    reference.write32(0x7b0ce8, 10);
                    reference.write32(0x12_0218, 6);
                    reference.write8(0xcedb4 + 6, 0);
                    reference.write32(0xced78, if seed % 2 == 0 { 0xcd053 } else { 0xccfb8 });
                    let mut actual = Ram(reference.0.clone());
                    let mut expected_regs: ExtRegisters = unsafe { core::mem::zeroed() };
                    expected_regs.esp = stack;
                    expected_regs.esi = seed;
                    expected_regs.edi = seed.rotate_left(7);
                    let mut actual_regs = expected_regs;
                    let mut expected_calls = Calls(Vec::new(), seed);
                    let mut actual_calls = Calls(Vec::new(), seed);
                    if mode != 0 {
                        PLAYER.ensure_init(&actual);
                    }
                    let before: Vec<u32> = (0..0x2000)
                        .step_by(4)
                        .map(|off| reference.read32(record + off))
                        .collect();
                    assert!(crate::render_lane2::scene_batch::run(
                        va,
                        &mut reference,
                        &mut expected_calls,
                        &mut expected_regs
                    ));
                    assert!(run_scene(
                        va,
                        &mut actual,
                        &mut actual_calls,
                        &mut actual_regs
                    ));
                    // x87 NaNs: compare the stack bit-exactly, the rest by value.
                    let split = |r: &ExtRegisters| {
                        let mut rest = *r;
                        rest.fpu.stack = [0.0; 8];
                        (r.fpu.stack.map(f64::to_bits), rest)
                    };
                    assert_eq!(
                        split(&actual_regs),
                        split(&expected_regs),
                        "mode {mode}, root {va:x}, seed {seed}"
                    );
                    assert_eq!(actual_calls.0, expected_calls.0);
                    assert_eq!(
                        actual.0, reference.0,
                        "mode {mode}, root {va:x}, seed {seed}"
                    );
                    record_writes[(va == 0xccab0) as usize] += (0..0x2000u32)
                        .step_by(4)
                        .zip(&before)
                        .filter(|(off, old)| reference.read32(record + off) != **old)
                        .count();
                    if mode == 0 {
                        assert!(!PLAYER.is_init());
                    } else {
                        for off in (0..0x2000).step_by(4) {
                            assert_eq!(
                                PLAYER.word(Slot::Record(off as usize / 4)),
                                actual.read32(record + off),
                                "setter mode {mode}, root {va:x}, seed {seed}, offset {off:x}"
                            );
                        }
                        assert_eq!(PLAYER.drift_count(), 0);
                    }
                }
            }
        }
        // Non-vacuous: the fixtures must actually change record words.
        std::eprintln!("scene record word changes per root {record_writes:?}");
        assert!(
            record_writes.iter().all(|&n| n > 0),
            "a root reached no record store: {record_writes:?}"
        );
        PLAYER.set_mode(0);
        PLAYER.reset();
    }

    /// Loader pointer transitions must be observed before a selected writer runs.
    /// This is a fabricated authority-contract test, not a gameplay reload trace.
    #[test]
    fn weapon_reset_follows_record_reallocation_and_null_interval() {
        use crate::abi::{ExtRegisters, GuestCall};
        struct Calls(Vec<u32>);
        impl GuestCall for Calls {
            fn call(&mut self, va: u32, _: &mut ExtRegisters) {
                self.0.push(va);
            }
        }
        for mode in [0, RETIRE_AUDIT, RETIRE_OWN] {
            PLAYER.reset();
            PLAYER.set_mode(mode);
            let mut actual = image();
            let mut reference = Ram(actual.0.clone());
            let stack = 0x18_0100;
            // Keep the owner alive across allocations and an absent-record interval.
            for (generation, record) in [0x10_0000u32, 0x11_0000, 0, 0x10_0000]
                .into_iter()
                .enumerate()
            {
                let weapon = if record == 0 {
                    0x16_0000
                } else {
                    record + 0x700
                };
                let shooter = if record == 0 {
                    0x17_0000
                } else {
                    record + 0x1200
                };
                for memory in [&mut actual, &mut reference] {
                    memory.write32(RECORD_CELL, record);
                    memory.write32(CONTROLLED_CELL, record);
                    memory.write32(STATS_CELL, 0x14_0000 + generation as u32 * 0x2000);
                    if record != 0 {
                        for off in (0..0x2000).step_by(4) {
                            memory.write32(record + off, 0xa5a5_0000 ^ off ^ generation as u32);
                        }
                    }
                    memory.write32(stack + 4, generation as u32);
                    memory.write32(stack + 8, weapon);
                    memory.write32(stack + 12, shooter);
                    memory.write32(stack + 16, 123);
                }
                let mut expected_regs: ExtRegisters = unsafe { core::mem::zeroed() };
                expected_regs.esp = stack;
                expected_regs.esi = 17;
                expected_regs.edi = 29;
                let mut actual_regs = expected_regs;
                let mut expected_calls = Calls(Vec::new());
                let mut actual_calls = Calls(Vec::new());
                assert!(crate::weapons_slice::run(
                    0x1b7bc0,
                    true,
                    &mut reference,
                    &mut expected_calls,
                    &mut expected_regs
                ));
                assert!(crate::leaf_lane::run(
                    0x1b7bc0,
                    &mut actual,
                    &mut actual_calls,
                    &mut actual_regs
                ));
                assert_eq!(
                    actual.0, reference.0,
                    "mode {mode}, generation {generation}"
                );
                assert_eq!(actual_regs, expected_regs);
                assert_eq!(actual_calls.0, expected_calls.0);
                assert_eq!(actual_calls.0, [0x1b6ec0]);
                if mode == 0 {
                    assert!(!PLAYER.is_init());
                } else {
                    assert_eq!(PLAYER.ptr(PTR_RECORD), record);
                    assert_eq!(PLAYER.ptr(PTR_STATS), actual.read32(STATS_CELL));
                    if record != 0 {
                        for off in (0..0x2000).step_by(4) {
                            assert_eq!(
                                PLAYER.word(Slot::Record(off as usize / 4)),
                                actual.read32(record + off),
                                "setter mode {mode}, generation {generation}, offset {off:x}"
                            );
                        }
                    }
                    assert_eq!(PLAYER.drift_count(), 0);
                }
            }
        }
        PLAYER.set_mode(0);
        PLAYER.reset();
    }

    #[test]
    fn wrapper_root_owned_only_mutants_affect_own_but_not_audit() {
        use crate::abi::{ExtRegisters, GuestCall};
        struct Calls;
        impl GuestCall for Calls {
            fn call(&mut self, _: u32, r: &mut ExtRegisters) {
                r.eax = 123;
            }
        }
        for va in WRAPPER_ROOTS {
            let mut baseline = image();
            let base = 0x10_0700;
            let stack = 0x18_0100;
            baseline.write32(stack + 4, base);
            let corrupt = if va == 0xe1200 {
                baseline.write32(stack + 8, 1f32.to_bits());
                baseline.write32(0x475c78, 1f32.to_bits());
                baseline.write32(0x4e8cec, 1);
                baseline.write32(0x4e794c, 1f32.to_bits());
                baseline.write32(base + 0xc, 0x3f80_0000);
                (0x70c, 0x4000_0000)
            } else {
                baseline.write32(stack + 8, 7);
                baseline.write32(0x790950, 1);
                (0x708, 1)
            };
            let mut expected = Ram(baseline.0.clone());
            let mut r: ExtRegisters = unsafe { core::mem::zeroed() };
            r.esp = stack;
            PLAYER.reset();
            PLAYER.set_mode(0);
            assert!(crate::leaf_lane::run(va, &mut expected, &mut Calls, &mut r));
            for mode in [RETIRE_OWN, RETIRE_AUDIT] {
                PLAYER.reset();
                PLAYER.set_mode(mode);
                let mut actual = Ram(baseline.0.clone());
                PLAYER.ensure_init(&actual);
                PLAYER.set_word(Slot::Record(corrupt.0 / 4), corrupt.1);
                r = unsafe { core::mem::zeroed() };
                r.esp = stack;
                assert!(crate::leaf_lane::run(va, &mut actual, &mut Calls, &mut r));
                if mode == RETIRE_OWN {
                    assert_ne!(actual.0, expected.0, "root {va:x}");
                } else {
                    assert_eq!(actual.0, expected.0, "root {va:x}");
                }
            }
        }
    }

    #[test]
    fn guard_hole_mutants_do_not_change_a_live_authority() {
        let m = image();
        assert_eq!(configure(RETIRE_AUDIT, |_| true), Ok(()));
        PLAYER.ensure_init(&m);
        PLAYER.set_word(Slot::Record(0), 0x2138);
        for hole in REQUIRED_ROOTS {
            assert_eq!(configure(RETIRE_OWN, |va| va != hole), Err(hole));
            assert_eq!(PLAYER.mode(), RETIRE_AUDIT);
            assert_eq!(PLAYER.word(Slot::Record(0)), 0x2138);
            assert!(PLAYER.is_init());
        }
        assert_eq!(configure(RETIRE_OWN, |_| true), Ok(()));
        for root in WRAPPER_ROOTS {
            assert!(crate::lane_inv::INV_ROOTS.contains(&root));
        }
    }

    #[test]
    fn owned_stats_setter_mutants_are_visible_without_a_noise_mask() {
        let mut m = image();
        PLAYER.set_mode(RETIRE_OWN);
        PLAYER.ensure_init(&m);
        // Every authoritative stats slot: full setter updates mirror and owner;
        // owned-only corruption must affect own reads and leave audit reads equal.
        for i in 0..(crate::leaf_wire::player::STATS_SIZE / 4) as usize {
            let slot = Slot::Stats(i);
            if !PLAYER.authoritative(slot) {
                continue;
            }
            let address = 0x14_0000 + 4 * i as u32;
            let value = 0x3f80_0000 ^ (i as u32 * 17);
            crate::leaf_wire::set_stat_at(&mut m, address, value);
            assert_eq!(get32(&m, address), value);
            assert_eq!(m.read32(address), value);
            PLAYER.set_word(slot, value ^ 0x0040_0000);
            assert_ne!(get32(&m, address), m.read32(address));
            PLAYER.set_mode(RETIRE_AUDIT);
            let before = PLAYER.drift_count();
            assert_eq!(get32(&m, address), value);
            assert_eq!(PLAYER.drift_count(), before + 1);
            assert_eq!(get32(&m, address), value);
            assert_eq!(PLAYER.drift_count(), before + 1);
            PLAYER.set_mode(RETIRE_OWN);
            // Mirror-only setter mutant: own retains the old word; audit sees it.
            m.write32(address, value ^ 1);
            assert_eq!(get32(&m, address), value);
            PLAYER.set_mode(RETIRE_AUDIT);
            assert_eq!(get32(&m, address), value ^ 1);
            PLAYER.set_mode(RETIRE_OWN);
        }
    }

    #[test]
    fn store_read_rebase_and_audit() {
        let mut m = image();
        m.write32(0x10_0010, 0x1122_3344);
        PLAYER.reset();
        PLAYER.set_mode(RETIRE_OWN);
        // sub-dword store merges into the owned word and writes the dword through
        assert!(store(&mut m, 0x10_0011, 0xaa, 1, 0));
        assert_eq!(m.read32(0x10_0010), 0x1122_aa44);
        assert_eq!(get32(&m, 0x10_0010), 0x1122_aa44);
        assert_eq!(get8(&m, 0x10_0013), 0x11);
        assert_eq!(get16(&m, 0x10_0012), 0x1122);
        // a bypassing guest store is invisible to own-mode readers
        m.write32(0x10_0010, 0xdead_beef);
        assert_eq!(get32(&m, 0x10_0010), 0x1122_aa44);
        // outside the extents: the guest value, and the store is refused (raw)
        m.write32(0x18_0000, 7);
        assert_eq!(get32(&m, 0x18_0000), 7);
        assert!(!store(&mut m, 0x18_0000, 1, 4, 0));
        // re-base: the record is reallocated, the owner follows the pointer cell and reloads the new body
        m.write32(0x11_0000 + 0x20, 0x5555);
        m.write32(RECORD_CELL, 0x11_0000);
        assert_eq!(get32(&m, 0x11_0020), 0x5555);
        assert_eq!(PLAYER.ptr(0), 0x11_0000);
        assert_eq!(PLAYER.rebase_count(), 1);
        // stats table store
        assert!(store(&mut m, 0x14_0008, 0x3f80_0000, 4, 0));
        assert_eq!(get32(&m, 0x14_0008), 0x3f80_0000);
        // audit: guest value returned, bypass counted once by the entry scan
        PLAYER.reset();
        PLAYER.set_mode(RETIRE_AUDIT);
        PLAYER.ensure_init(&m);
        m.write32(0x11_0040, 9);
        assert_eq!(get32(&m, 0x11_0040), 9);
        assert_eq!(PLAYER.drift_count(), 1);
        m.write32(0x11_0044, 1);
        audit_entry(&m, 0x1234);
        assert_eq!(PLAYER.drift_count(), 2);
        audit_entry(&m, 0x1234);
        assert_eq!(PLAYER.drift_count(), 2);
        PLAYER.set_mode(0);
        PLAYER.reset();
    }
}
